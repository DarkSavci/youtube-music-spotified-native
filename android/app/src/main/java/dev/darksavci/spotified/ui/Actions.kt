package dev.darksavci.spotified.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.navigation.NavController
import androidx.navigation.NavHostController
import dev.darksavci.spotified.Graph
import dev.darksavci.spotified.api.Album
import dev.darksavci.spotified.api.Artist
import dev.darksavci.spotified.api.ArtistRef
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.Playlist
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.Services
import dev.darksavci.spotified.ui.screens.EpisodeProgress
import dev.darksavci.spotified.ui.screens.episodeResumeAt
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Routes, as the navigation graph names them. Ids are encoded when built. */
object Routes {
    const val HOME = "home"
    const val SEARCH = "search"
    const val SEARCH_ROUTE = "search?q={q}"
    const val LIBRARY = "library"
    const val ALBUM = "album/{id}"
    const val ARTIST = "artist/{id}"
    const val PLAYLIST = "playlist/{id}"
    const val PODCAST = "podcast/{id}"
    const val BROWSE = "browse/{id}?params={params}&title={title}"
    const val LIKED = "liked"
    const val LIKED_ROUTE = "liked?shuffle={shuffle}"
    const val HISTORY = "history"
    const val STATS = "stats"
    const val SETTINGS = "settings"
    const val SIGN_IN = "sign-in"
    /** Sign-in as the graph knows it; with new=1 it makes another account, and only once it has signed in. */
    const val SIGN_IN_ROUTE = "sign-in?new={new}"
    const val SIGN_IN_NEW = "sign-in?new=1"
    const val BLOCKED = "blocked"
    const val WHATS_NEW = "whats-new"
    const val LICENCES = "licences"
    const val STAT_ROUTE = "stat/{kind}?id={id}"
    const val EQUALIZER = "equalizer"
    const val DOWNLOADS = "downloads"
    const val LOCAL = "local"
    const val TOGETHER = "together"
    const val IMPORT = "import"
    const val RECAP = "recap"
    const val PLAYLIST_ADD = "playlist-add/{id}?title={title}"

    fun album(id: String) = "album/${Uri.encode(id)}"
    fun artist(id: String) = "artist/${Uri.encode(id)}"
    fun playlist(id: String) = "playlist/${Uri.encode(id)}"
    fun podcast(id: String) = "podcast/${Uri.encode(id)}"
    fun stat(kind: String, id: String) = "stat/$kind?id=${Uri.encode(id)}"
    fun playlistAdd(id: String, title: String) = "playlist-add/${Uri.encode(id)}?title=${Uri.encode(title)}"
    fun browse(id: String, params: String = "", title: String = "") =
        "browse/${Uri.encode(id)}?params=${Uri.encode(params)}&title=${Uri.encode(title)}"

    /** The route pattern of a tab's root page, which is what the back stack knows it by. */
    fun rootOf(tab: String) = if (tab == SEARCH) SEARCH_ROUTE else tab
}

/** What tapping a tab should do. */
internal sealed interface TabTap {
    /** It is the tab showing, at its root: go to the top. */
    object ToTop : TabTap
    /** It is the tab showing, on a page under its root: go back to the root. */
    object ToRoot : TabTap
    /** Another tab. The one left keeps its pages if [keepLeft]. */
    data class Switch(val keepLeft: Boolean) : TabTap
}

/**
 * Leaving Home for another tab drops the pages above it instead of saving them,
 * as [switchTab] never restores Home. The other tabs come back as they were left.
 */
internal fun tabTap(tab: String, currentTab: String, currentRoute: String?): TabTap = when {
    tab != currentTab -> TabTap.Switch(keepLeft = currentTab != Routes.HOME)
    currentRoute == Routes.rootOf(tab) -> TabTap.ToTop
    else -> TabTap.ToRoot
}

/**
 * Moves to another tab's root, or to where it was left. A pop that saves also files
 * what it saved under the destination it popped to, so after leaving a tab that way
 * Home itself names that tab's stack: restoring by Home's name would bring the tab
 * just left back up. Home is the bottom of the stack, and popping to it is all that
 * reaching it takes.
 */
internal fun NavController.switchTab(tab: String, keepLeft: Boolean) {
    navigate(tab) {
        popUpTo(graph.startDestinationId) { saveState = keepLeft }
        launchSingleTop = true
        restoreState = tab != Routes.HOME
    }
}

/** Something a write changed on the account, which the pages showing it should load again. */
sealed interface Change {
    /** The list of what is in the library: anything made, deleted, saved, pinned or moved. */
    object Library : Change
    /** The folders themselves. */
    object Folders : Change
    /** The liked songs. Root says this itself when a like is written. */
    object Liked : Change
    /** One playlist's songs or details. Its id is compared without the browse id's "VL". */
    class Playlist(id: String) : Change {
        val id = id.removePrefix("VL")
        override fun equals(other: Any?) = other is Playlist && other.id == id
        override fun hashCode() = id.hashCode()
    }
    /** All of it, as when the account itself changes. Every page following anything follows this. */
    object Everything : Change
}

/**
 * The app's record of what has been changed. It counts rather than announces,
 * so a page that was under another when a write happened still sees it when it
 * returns. It lives as long as the process, as the pages' loaders do.
 */
object Changes {
    private val versions = MutableStateFlow<Map<Change, Long>>(emptyMap())

    fun say(vararg what: Change) {
        versions.update { old -> what.fold(old) { map, change -> map + (change to (map[change] ?: 0L) + 1L) } }
    }

    /** A number that moves when anything in [what] changes. */
    fun version(what: Set<Change>): Long = sum(versions.value, what)

    fun follow(what: Set<Change>): Flow<Long> = versions.map { sum(it, what) }.distinctUntilChanged()

    private fun sum(map: Map<Change, Long>, what: Set<Change>): Long =
        (what + Change.Everything).sumOf { map[it] ?: 0L }
}

/**
 * Which address each page was opened by, to know a second tap on a link from a
 * new link. A page opened from one of its own kind, an artist from an artist,
 * is a new page; `launchSingleTop` would swap the page under it instead, since it
 * goes by the destination and not its arguments, and Back would skip one.
 */
internal class OpenedPages {
    private val routes = object : LinkedHashMap<String, String>() {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, String>?) = size > KEPT
    }

    fun isShowing(entry: String?, route: String) = entry != null && routes[entry] == route

    fun note(entry: String?, route: String) {
        if (entry != null) routes[entry] = route
    }

    private companion object {
        const val KEPT = 64
    }
}

/** A question for Root to ask before something that cannot be taken back. */
class Confirm(val title: String, val text: String, val action: String, val danger: Boolean, val dismiss: String, val run: () -> Unit)

/** A message, perhaps with a button such as Undo. */
class Notice(val message: String, val action: String? = null, val run: (() -> Unit)? = null)

/** What a menu was opened on. */
sealed interface MenuTarget {
    /** A song; [playlist] and [itemId] when it sits in an editable playlist, to remove it. */
    data class Song(val track: Track, val playlist: Playlist? = null, val queueIndex: Int? = null) : MenuTarget
    data class Entity(val item: Item) : MenuTarget
    data class AddTo(val tracks: List<Track>) : MenuTarget
    object SleepTimer : MenuTarget
    object Speed : MenuTarget
    object Queue : MenuTarget
    /** The Create tab's choices. */
    object Create : MenuTarget
    /** Filing something from the library in a folder, or taking it out. */
    data class MoveToFolder(val kind: LibraryKind, val id: String, val title: String) : MenuTarget
    /** Asks a new playlist's name, and opens it once made. */
    object NewPlaylist : MenuTarget
    /** The links there are to copy or send for something. */
    data class Share(val what: Shared) : MenuTarget
    /** Blocks or unblocks a song, its artists or its album. */
    data class Block(val track: Track) : MenuTarget
    /** A song by several artists: which one to go to. */
    data class Artists(val track: Track) : MenuTarget
    /** An album by several artists: which one to go to. */
    data class ArtistsOf(val artists: List<ArtistRef>) : MenuTarget
}

/** Something whose link can be shared. A playlist is YouTube Music's alone, so it has no links elsewhere. */
sealed interface Shared {
    data class Song(val track: Track) : Shared
    data class AlbumOf(val album: Album) : Shared
    data class ArtistOf(val id: String, val name: String) : Shared
    data class PlaylistOf(val id: String, val title: String) : Shared
}

/**
 * What a screen can do, in one place, so opening, playing and menus behave
 * the same everywhere.
 */
class Actions(val graph: Graph, val nav: NavHostController, private val context: Context) {
    var menu by mutableStateOf<MenuTarget?>(null)
    var nowPlayingOpen by mutableStateOf(false)
    var lyricsOpen by mutableStateOf(false)
    /** The side menu Home's picture opens. */
    var drawerOpen by mutableStateOf(false)
    /** The listener's Reduce motion, for the page transitions and the side menu. */
    var reduceMotion by mutableStateOf(false)
    /** The question being asked, which Root shows. See [confirm]. */
    var confirming by mutableStateOf<Confirm?>(null)

    val playback get() = graph.playback

    /**
     * Opens a page, on top of the tab showing. A tab's root is never pushed
     * inside another tab, or that tab's saved pages would be found by it, so
     * asking for one switches to its tab; a search with a query starts afresh there.
     */
    fun go(route: String) {
        nowPlayingOpen = false
        lyricsOpen = false
        when {
            route == Routes.HOME || route == Routes.SEARCH || route == Routes.LIBRARY -> switchTo(route)
            route.startsWith(Routes.SEARCH + "?") -> {
                val left = currentTab()
                betweenTabs {
                    // Another stack saved for Search would be restored in place of this one later.
                    nav.clearBackStack(Routes.SEARCH_ROUTE)
                    nav.navigate(route) {
                        popUpTo(nav.graph.startDestinationId) { saveState = left == Routes.LIBRARY }
                    }
                }
            }
            // A second tap arriving before the page does must not open it twice.
            opened.isShowing(nav.currentBackStackEntry?.id, route) -> Unit
            else -> {
                nav.navigate(route)
                opened.note(nav.currentBackStackEntry?.id, route)
            }
        }
    }

    private val opened = OpenedPages()

    /** Whether the last move was between tabs, for the page transition: Back and a page opened are not. */
    internal var lastMoveWasTab = false
        private set

    private var movingTabs = false

    init {
        nav.addOnDestinationChangedListener { _, _, _ -> lastMoveWasTab = movingTabs }
    }

    private inline fun betweenTabs(move: () -> Unit) {
        movingTabs = true
        try {
            move()
        } finally {
            movingTabs = false
        }
    }

    /** The tab that owns what shows: a page under a tab counts as that tab. */
    fun currentTab(): String = when {
        inStack(Routes.SEARCH_ROUTE) -> Routes.SEARCH
        inStack(Routes.LIBRARY) -> Routes.LIBRARY
        else -> Routes.HOME
    }

    private fun inStack(route: String) = runCatching { nav.getBackStackEntry(route) }.isSuccess

    /**
     * A tap on a tab. Another tab comes back as it was left; the tab showing goes
     * back to its root from a page under it, and to its top from its root. Going
     * back to the root pops rather than navigates, since restoring would only
     * bring back the page being left.
     */
    fun tabTo(tab: String) {
        nowPlayingOpen = false
        lyricsOpen = false
        if (!switchTo(tab)) tabScrolls[tab]?.invoke()
    }

    /** Moves to [tab]'s root, or to where it was left; false when already at its root. */
    private fun switchTo(tab: String): Boolean {
        val tap = tabTap(tab, currentTab(), nav.currentBackStackEntry?.destination?.route)
        when (tap) {
            TabTap.ToTop -> return false
            TabTap.ToRoot -> nav.popBackStack(Routes.rootOf(tab), inclusive = false)
            is TabTap.Switch -> betweenTabs { nav.switchTab(tab, tap.keepLeft) }
        }
        return true
    }

    private val tabScrolls = mutableMapOf<String, () -> Unit>()

    internal fun followTabScroll(tab: String, scroll: () -> Unit) {
        tabScrolls[tab] = scroll
    }

    internal fun forgetTabScroll(tab: String, scroll: () -> Unit) {
        if (tabScrolls[tab] === scroll) tabScrolls.remove(tab)
    }

    fun open(item: Item) = when (item) {
        is Item.Song -> play(listOf(item.track), 0, item.track.title)
        is Item.Episode -> playEpisode(listOf(item.track), 0, item.track.title)
        is Item.AlbumItem -> go(Routes.album(item.album.id))
        is Item.ArtistItem -> go(Routes.artist(item.artist.id))
        is Item.PlaylistItem -> go(Routes.playlist(item.playlist.id))
        is Item.PodcastItem -> go(Routes.podcast(item.podcast.id))
    }

    fun openAlbumOf(track: Track) {
        track.album?.id?.takeIf { it.isNotEmpty() }?.let { go(Routes.album(it)) }
    }

    fun openArtistOf(track: Track) {
        track.artists.firstOrNull { it.id.isNotEmpty() }?.let { go(Routes.artist(it.id)) }
    }

    fun play(tracks: List<Track>, index: Int, origin: String) = playback.play(tracks, index, origin)

    /** An episode carries on: from where it was left, and if it is the one in hand, from where it is. */
    fun playEpisode(episodes: List<Track>, index: Int, origin: String) {
        val episode = episodes.getOrNull(index) ?: return
        if (playback.projection.value.state.current?.id == episode.id) return playback.resume()
        val at = episodeResumeAt(EpisodeProgress.of(context, episode.id), episode.durationMs)
        if (at > 0) playback.playFrom(episodes, index, origin, at) else play(episodes, index, origin)
    }

    fun playAlbum(album: Album, index: Int = 0) =
        play(album.tracks.map { if (it.artwork.isEmpty()) it.copy(artwork = album.artwork) else it }, index, album.title)

    fun menuFor(track: Track, playlist: Playlist? = null, queueIndex: Int? = null) {
        menuBack.clear()
        if (queueIndex != null) menuBack.addLast(MenuTarget.Queue)
        menu = MenuTarget.Song(track, playlist, queueIndex)
    }

    /** The menus to come back to, the last opened last. */
    private val menuBack = ArrayDeque<MenuTarget>()

    /** Opens [next] from the menu [here], which comes back when [next] is closed. */
    fun menuFrom(here: MenuTarget, next: MenuTarget) {
        menuBack.addLast(here)
        menu = next
    }

    /** The menu was dismissed: goes back to the one it was opened from, if any. */
    fun closeMenu() {
        menu = menuBack.removeLastOrNull()
    }

    /** What the menu is for has been done: closes it, back to the queue if that is where it began. */
    fun finishMenu() {
        menu = if (MenuTarget.Queue in menuBack) MenuTarget.Queue else null
        menuBack.clear()
    }

    /** Closes the menu and any it was opened from, as when going to a page. */
    fun closeMenus() {
        menuBack.clear()
        menu = null
    }

    fun menuFor(item: Item) {
        menu = when (item) {
            is Item.Song -> MenuTarget.Song(item.track)
            is Item.Episode -> MenuTarget.Song(item.track)
            else -> MenuTarget.Entity(item)
        }
    }

    fun addTo(tracks: List<Track>) {
        menu = MenuTarget.AddTo(tracks)
    }

    /** Shares the YouTube Music address, through the system's share sheet. */
    fun share(title: String, url: String) {
        val send = Intent(Intent.ACTION_SEND).setType("text/plain")
            .putExtra(Intent.EXTRA_SUBJECT, title)
            .putExtra(Intent.EXTRA_TEXT, url)
        context.startActivity(Intent.createChooser(send, "Share").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }

    fun say(message: String) = playback.say(message)

    private val _notices = MutableSharedFlow<Notice>(extraBufferCapacity = 8)

    /** Messages with a button, which Root shows beside [say]'s. */
    val notices: SharedFlow<Notice> = _notices

    /** A message with an Undo button; [undo] runs if it is pressed, on the main thread. */
    fun sayWithUndo(message: String, undo: () -> Unit) {
        // Root and an open menu both show it, and it is undone once whichever is pressed.
        var undone = false
        _notices.tryEmit(
            Notice(message, "Undo") {
                if (!undone) {
                    undone = true
                    undo()
                }
            },
        )
    }

    /** Asks before something that cannot be taken back, and runs [run] on yes. Safe from a menu that is closing. */
    fun confirm(title: String, text: String, action: String, danger: Boolean = true, dismiss: String = "Cancel", run: () -> Unit) {
        confirming = Confirm(title, text, action, danger, dismiss, run)
    }

    fun launch(block: suspend () -> Unit) {
        graph.scope.launch { runCatching { block() }.onFailure { say(it.message ?: "That did not work.") } }
    }

    /** Says that [what] has changed, so the pages showing it load again. */
    fun changed(vararg what: Change) = Changes.say(*what)

    /** A playlist's songs or details changed, and with them its line in the library. */
    fun playlistChanged(id: String) = Changes.say(Change.Playlist(id), Change.Library)

    /**
     * A write on the account: runs [block] off the main thread, says [done] if
     * given, and on success says what changed. A failure is said as a message and
     * changes nothing.
     */
    fun write(vararg changes: Change, done: String? = null, block: suspend () -> Unit) {
        graph.scope.launch {
            runCatching { block() }.fold(
                {
                    Changes.say(*changes)
                    if (done != null) say(done)
                },
                { say(it.message ?: "That could not be saved.") },
            )
        }
    }

    /** Copies [text] to the clipboard and says [message]. */
    fun copy(text: String, message: String = "Copied to clipboard") {
        context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Spotified", text))
        say(message)
    }

    /** Finds [wanted] on another service, off the main thread, and copies the link. */
    fun copyLinkOn(service: Services.Service, wanted: Services.Wanted) {
        val noun = when (wanted) {
            is Services.Wanted.Song -> "song"
            is Services.Wanted.Album -> "album"
            is Services.Wanted.Artist -> "artist"
        }
        say("Looking for the $noun on ${service.label}…")
        graph.scope.launch { find(service, wanted, noun) }
    }

    fun copyLinkOn(service: Services.Service, track: Track) = copyLinkOn(
        service,
        Services.Wanted.Song(track.title, track.artists.firstOrNull()?.name.orEmpty(), track.album?.name.orEmpty()),
    )

    fun copyLinkOn(service: Services.Service, artist: Artist) =
        copyLinkOn(service, Services.Wanted.Artist(artist.name, artist.id))

    /** An album is looked for by some of its songs; one known only by name has its page read first. */
    fun copyLinkOn(service: Services.Service, album: Album) {
        if (album.tracks.isNotEmpty()) return copyLinkOn(service, wantedOf(album))
        say("Looking for the album on ${service.label}…")
        graph.scope.launch {
            runCatching { graph.api.album(album.id) }.fold(
                { find(service, wantedOf(it), "album") },
                { say("Couldn't read the album to find it on ${service.label}: ${it.message.orEmpty()}") },
            )
        }
    }

    private fun wantedOf(album: Album) = Services.Wanted.Album(
        album.title, album.artists.firstOrNull()?.name.orEmpty(), album.tracks.take(ALBUM_SONGS).map { it.title },
    )

    private suspend fun find(service: Services.Service, wanted: Services.Wanted, noun: String) {
        runCatching { Services.find(graph.http, service, wanted) }.fold(
            { link ->
                if (link != null) copy(link, "${service.label} link copied to clipboard")
                else say("Couldn't find this $noun on ${service.label}")
            },
            { say("Couldn't reach ${service.label}: ${it.message.orEmpty()}") },
        )
    }

    companion object {
        /** How many of an album's songs it is looked for by on another service. */
        private const val ALBUM_SONGS = 3

        fun songLink(id: String) = "https://music.youtube.com/watch?v=$id"
        fun albumLink(id: String) = "https://music.youtube.com/browse/$id"
        fun playlistLink(id: String) = "https://music.youtube.com/playlist?list=${id.removePrefix("VL")}"
        fun artistLink(id: String) = "https://music.youtube.com/channel/$id"
    }
}

val LocalActions = staticCompositionLocalOf<Actions> { error("no actions") }

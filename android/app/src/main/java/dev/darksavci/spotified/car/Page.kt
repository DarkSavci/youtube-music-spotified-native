package dev.darksavci.spotified.car

import androidx.annotation.DrawableRes
import androidx.car.app.CarContext
import androidx.car.app.Screen
import androidx.car.app.ScreenManager
import androidx.car.app.constraints.ConstraintManager
import androidx.car.app.model.Action
import androidx.car.app.model.CarColor
import androidx.car.app.model.CarIcon
import androidx.car.app.model.GridItem
import androidx.car.app.model.Row
import androidx.core.graphics.drawable.IconCompat
import androidx.lifecycle.lifecycleScope
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Album
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.api.url
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.playback.ArtworkProvider
import kotlinx.coroutines.launch

/**
 * What every car screen shares: loading its content once, pictures through
 * the app's artwork provider, and how a song plays or a collection opens.
 */
abstract class Page(carContext: CarContext) : Screen(carContext) {
    protected val graph = carContext.graph

    /** Whether the content is still on its way. */
    protected var loading = true
        private set

    /** What went wrong loading, said in place of the content. */
    protected var failure: String? = null
        private set

    /** Loads [block] once, then draws the screen again. */
    protected fun load(block: suspend () -> Unit) {
        loading = true
        lifecycleScope.launch {
            failure = runCatching { block() }.exceptionOrNull()?.let { it.message ?: "Something went wrong." }
            loading = false
            invalidate()
        }
    }

    /** How many rows or tiles the car shows in one list. */
    protected fun limit(type: Int = ConstraintManager.CONTENT_LIMIT_TYPE_LIST): Int = runCatching {
        (carContext.getCarService(CarContext.CONSTRAINT_SERVICE) as ConstraintManager).getContentLimit(type)
    }.getOrDefault(50)

    protected fun icon(@DrawableRes res: Int): CarIcon = CarIcon.Builder(IconCompat.createWithResource(carContext, res)).build()

    /** A cover, served to the car by the app: the car cannot fetch the web. */
    protected fun art(artwork: List<Artwork>, size: Int = 300): CarIcon? =
        ArtworkProvider.uriFor(artwork.url(size))?.let { CarIcon.Builder(IconCompat.createWithContentUri(it)).build() }

    /** Plays [tracks] from [index], and shows what plays. */
    protected fun play(tracks: List<Track>, index: Int, origin: String) {
        if (tracks.isEmpty()) return
        graph.playback.play(tracks, index, origin)
        showNowPlaying(screenManager, carContext)
    }

    protected fun shuffle(tracks: List<Track>, origin: String) {
        if (tracks.isEmpty()) return
        graph.playback.shuffle(tracks, origin)
        showNowPlaying(screenManager, carContext)
    }

    /** A song in a list: choosing it plays the list from there. */
    protected fun songRow(tracks: List<Track>, index: Int, origin: String, withArt: Boolean = true): Row {
        val track = tracks[index]
        return Row.Builder()
            .setTitle(track.title.ifEmpty { "Unknown" })
            .apply { if (track.artistNames.isNotEmpty()) addText(track.artistNames) }
            .apply { if (withArt) art(track.artwork, 120)?.let { setImage(it, Row.IMAGE_TYPE_LARGE) } }
            .setOnClickListener { play(tracks, index, origin) }
            .build()
    }

    /** One of a shelf's items as a tile: a song plays its shelf, the rest open. */
    protected fun tile(items: List<Item>, item: Item, origin: String): GridItem? {
        val songs = items.mapNotNull(::trackOf)
        val (subtitle, click) = when (item) {
            is Item.Song, is Item.Episode -> {
                val track = trackOf(item)!!
                track.artistNames to { play(songs, songs.indexOf(track), origin) }
            }
            is Item.AlbumItem -> albumLine(item.album) to { open(Collection.Album(item.album.id, item.title)) }
            is Item.PlaylistItem -> item.playlist.owner to { open(Collection.Playlist(item.playlist.id, item.title)) }
            is Item.ArtistItem -> "Artist" to { open(Collection.Artist(item.artist.id, item.title)) }
            is Item.PodcastItem -> item.podcast.author to { open(Collection.Podcast(item.podcast.id, item.title)) }
        }
        val image = art(item.artwork) ?: icon(R.drawable.ic_music)
        return GridItem.Builder()
            .setTitle(item.title.ifEmpty { "Unknown" })
            .apply { if (subtitle.isNotEmpty()) setText(subtitle) }
            .setImage(image, GridItem.IMAGE_TYPE_LARGE)
            .setOnClickListener(click)
            .build()
    }

    protected fun albumLine(album: Album): String =
        listOf(album.year, album.artists.joinToString { it.name }).filter { it.isNotEmpty() }.joinToString(" • ")

    protected fun open(collection: Collection) {
        // The car allows five screens deep; a long walk starts again from Home.
        if (screenManager.stackSize >= MAX_DEPTH - 1) screenManager.popToRoot()
        screenManager.push(CollectionScreen(carContext, collection))
    }

    /** The way to Now Playing every browsing screen must offer. */
    protected fun nowPlayingAction(): Action = fab(R.drawable.ic_audio_lines) { showNowPlaying(screenManager, carContext) }

    /** A floating button: the car allows an icon only, on a colour of its own; the app's red marks the main one. */
    protected fun fab(@DrawableRes res: Int, primary: Boolean = false, onClick: () -> Unit): Action = Action.Builder()
        .setIcon(icon(res))
        .setBackgroundColor(if (primary) ACCENT else SURFACE)
        .setOnClickListener(onClick)
        .build()

    protected fun trackOf(item: Item): Track? = when (item) {
        is Item.Song -> item.track
        is Item.Episode -> item.track
        else -> null
    }

    companion object {
        const val MAX_DEPTH = 5
        private val ACCENT = CarColor.createCustom(0xFFFF0033.toInt(), 0xFFFF0033.toInt())
        private val SURFACE = CarColor.createCustom(0xFF383838.toInt(), 0xFF383838.toInt())
    }
}

/** Shows Now Playing, over what is there, or by going back to it if it is open already. */
fun showNowPlaying(screens: ScreenManager, carContext: CarContext) {
    if (screens.top is NowPlayingScreen) return
    if (screens.stackSize >= Page.MAX_DEPTH - 1) screens.popToRoot()
    screens.push(NowPlayingScreen(carContext))
}

/** What a collection screen shows. */
sealed interface Collection {
    val id: String
    val title: String

    data class Album(override val id: String, override val title: String) : Collection
    data class Playlist(override val id: String, override val title: String) : Collection
    data class Artist(override val id: String, override val title: String) : Collection
    data class Podcast(override val id: String, override val title: String) : Collection

    /** Songs already at hand, such as one of Home's shelves. */
    data class Songs(override val title: String, val tracks: List<Track>) : Collection {
        override val id get() = ""
    }
}

package dev.darksavci.spotified.data

import dev.darksavci.spotified.api.Api
import dev.darksavci.spotified.api.Track
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.serialization.Serializable

/**
 * What the listener never wants played: songs, artists and albums. A port of
 * crates/app/src/blocked.rs. The list is kept with the settings and told to the
 * core, which is what steps over a blocked song as the queue moves; here it is
 * only looked up, to say which a menu's entry would do and to list them.
 */
enum class BlockKind(val noun: String) { Song("Song"), Artist("Artist"), Album("Album") }

/**
 * One thing blocked: its id, and the name to show it by. [songs] are an album's,
 * once its page has been read: each is blocked while the album is, since a song
 * does not always say what album it is on.
 */
@Serializable
data class BlockEntry(val id: String = "", val name: String = "", val songs: List<String> = emptyList())

@Serializable
data class Blocked(
    val songs: List<BlockEntry> = emptyList(),
    val artists: List<BlockEntry> = emptyList(),
    val albums: List<BlockEntry> = emptyList(),
) {
    fun of(kind: BlockKind): List<BlockEntry> = when (kind) {
        BlockKind.Song -> songs
        BlockKind.Artist -> artists
        BlockKind.Album -> albums
    }

    private fun with(kind: BlockKind, list: List<BlockEntry>) = when (kind) {
        BlockKind.Song -> copy(songs = list)
        BlockKind.Artist -> copy(artists = list)
        BlockKind.Album -> copy(albums = list)
    }

    val isEmpty: Boolean get() = songs.isEmpty() && artists.isEmpty() && albums.isEmpty()

    /** Whether this very song, artist or album is blocked. */
    fun has(kind: BlockKind, id: String): Boolean {
        val wanted = key(kind, id)
        return wanted.isNotEmpty() && of(kind).any { key(kind, it.id) == wanted }
    }

    /** Whether a song is kept from playing: blocked itself, or by its album or one of its artists. */
    fun stops(track: Track): Boolean {
        if (isEmpty) return false
        return has(BlockKind.Song, track.id) ||
            albums.any { track.id in it.songs } ||
            track.album?.let { has(BlockKind.Album, it.id) } == true ||
            track.artists.any { has(BlockKind.Artist, it.id) }
    }

    /** Blocks it, or lets it play again; the same list when nothing changes. Nothing without an id is blocked. */
    fun set(kind: BlockKind, id: String, name: String, blocked: Boolean): Blocked {
        val wanted = key(kind, id)
        if (wanted.isEmpty() || has(kind, wanted) == blocked) return this
        val list = of(kind)
        return with(kind, if (blocked) list + BlockEntry(wanted, name) else list.filter { key(kind, it.id) != wanted })
    }

    /** Notes the songs a blocked album holds. An album that is not blocked, or came back empty, teaches nothing. */
    fun learnAlbum(id: String, found: List<String>): Blocked {
        if (found.isEmpty() || albums.none { it.id == id }) return this
        return copy(albums = albums.map { if (it.id == id) it.copy(songs = found) else it })
    }

    /** The blocked albums whose songs are not known yet. */
    val albumsUnread: List<String> get() = albums.filter { it.songs.isEmpty() }.map { it.id }

    /** The song ids as the core takes them: a blocked album's songs among the songs. */
    val coreTracks: List<String> get() = songs.map { it.id } + albums.flatMap { it.songs }
    val coreArtists: List<String> get() = artists.map { it.id }
    val coreAlbums: List<String> get() = albums.map { it.id }
}

/** The channel an artist's id names. One from the library arrives wrapped ("MPLA" before the channel's id), and is the same artist. */
private fun key(kind: BlockKind, id: String): String =
    if (kind == BlockKind.Artist && id.startsWith("MPLAUC")) id.removePrefix("MPLA") else id

/** What is blocked, with the names to show it by. */
val SettingsStore.blocked: Flow<Blocked> get() = settings.map { it.blocked }.distinctUntilChanged()

/** Blocks something, or lets it play again. The change reaches the core through [Settings.forCore]. */
suspend fun SettingsStore.setBlocked(kind: BlockKind, id: String, name: String, blocked: Boolean) {
    update { it.copy(blocked = it.blocked.set(kind, id, name, blocked)) }
}

/** Reads the page of each blocked album whose songs are not known yet, so that its songs are blocked wherever they turn up. */
suspend fun SettingsStore.learnBlockedAlbums(api: Api) {
    for (id in blocked.first().albumsUnread) {
        val songs = runCatching { api.album(id).tracks.map { it.id } }.getOrNull() ?: continue
        update { it.copy(blocked = it.blocked.learnAlbum(id, songs)) }
    }
}

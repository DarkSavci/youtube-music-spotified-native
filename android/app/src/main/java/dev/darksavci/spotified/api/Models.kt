package dev.darksavci.spotified.api

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/*
 * The core's JSON, as Kotlin types. They mirror crates/client/src/models.rs,
 * which mirrors core/internal/domain/types.go. Go leaves empty values out and
 * writes a nil slice as null, so every field has a default and the decoder
 * reads null as that default.
 */

val CoreJson = Json {
    ignoreUnknownKeys = true
    coerceInputValues = true
    explicitNulls = false
    encodeDefaults = true
}

@Serializable
data class Artwork(val url: String = "", val width: Int = 0, val height: Int = 0)

/**
 * The address of a picture at least [minWidth] wide. Google's image hosts
 * take the size in the URL, so one is asked for at the size drawn.
 */
fun List<Artwork>.url(minWidth: Int): String? {
    val chosen = firstOrNull { it.width >= minWidth } ?: lastOrNull() ?: return null
    return resized(chosen.url, minWidth) ?: chosen.url
}

private fun resized(url: String, size: Int): String? {
    val at = url.lastIndexOf('=')
    if (at < 0) return null
    val base = url.substring(0, at)
    val spec = url.substring(at + 1)
    val parts = spec.split('-', limit = 3)
    val width = parts.getOrNull(0)?.removePrefix("w")
    val height = parts.getOrNull(1)?.removePrefix("h")
    val digits = { s: String? -> !s.isNullOrEmpty() && s.all(Char::isDigit) }
    return when {
        parts.getOrNull(0)?.startsWith("w") == true && parts.getOrNull(1)?.startsWith("h") == true &&
            digits(width) && digits(height) ->
            // A wide banner asked for square comes back padded with black; it keeps its own shape, and is cropped where it shows.
            if (width == height) "$base=w$size-h$size-${parts.getOrNull(2) ?: "l90-rj"}" else null
        spec.startsWith("s") -> "$base=s$size"
        else -> null
    }
}

@Serializable
data class ArtistRef(val id: String = "", val name: String = "")

@Serializable
data class AlbumRef(val id: String = "", val name: String = "")

@Serializable
data class Track(
    val id: String = "",
    val title: String = "",
    val artists: List<ArtistRef> = emptyList(),
    val album: AlbumRef? = null,
    val durationMs: Long = 0,
    val artwork: List<Artwork> = emptyList(),
    val explicit: Boolean = false,
    val isVideo: Boolean = false,
    val playable: Boolean = true,
    val playCount: String = "",
    val playlistItemId: String = "",
) {
    val artistNames: String get() = artists.joinToString(", ") { it.name }
}

@Serializable
data class Shelf(
    val title: String = "",
    @Serializable(with = ItemListSerializer::class) val items: List<Item> = emptyList(),
    val showAllId: String = "",
    val showAllParams: String = "",
)

@Serializable
data class Album(
    val id: String = "",
    val title: String = "",
    val artists: List<ArtistRef> = emptyList(),
    val year: String = "",
    val trackCount: Int = 0,
    val durationMs: Long = 0,
    val artwork: List<Artwork> = emptyList(),
    val explicit: Boolean = false,
    val tracks: List<Track> = emptyList(),
    @SerialName("type") val kind: String = "",
    val artistArtwork: List<Artwork> = emptyList(),
    val description: String = "",
    val shelves: List<Shelf> = emptyList(),
)

@Serializable
data class BrowseLink(val id: String = "", val params: String = "")

@Serializable
data class Artist(
    val id: String = "",
    val name: String = "",
    val artwork: List<Artwork> = emptyList(),
    val subscribers: String = "",
    val monthlyListeners: String = "",
    val following: Boolean = false,
    val description: String = "",
    val descriptionUrl: String = "",
    val radioId: String = "",
    val radioSeed: String = "",
    val radioParams: String = "",
    val shuffleId: String = "",
    val shuffleSeed: String = "",
    val shuffleParams: String = "",
    val songsId: String = "",
    val albumsMore: BrowseLink? = null,
    val singlesMore: BrowseLink? = null,
    val topTracks: List<Track> = emptyList(),
    val albums: List<Album> = emptyList(),
    val singles: List<Album> = emptyList(),
    val related: List<Artist> = emptyList(),
)

@Serializable
data class Playlist(
    val id: String = "",
    val title: String = "",
    val description: String = "",
    val owner: String = "",
    val trackCount: Int = 0,
    val durationMs: Long = 0,
    val artwork: List<Artwork> = emptyList(),
    val editable: Boolean = false,
    val tracks: List<Track> = emptyList(),
)

@Serializable
data class PlaylistPage(val playlist: Playlist = Playlist(), val next: String = "")

@Serializable
data class Podcast(
    val id: String = "",
    val title: String = "",
    val author: String = "",
    val description: String = "",
    val artwork: List<Artwork> = emptyList(),
    val episodes: List<Track> = emptyList(),
)

/** Something a shelf or a search can hold. */
sealed interface Item {
    val artwork: List<Artwork>
    val title: String

    data class Song(val track: Track) : Item {
        override val artwork get() = track.artwork
        override val title get() = track.title
    }

    data class AlbumItem(val album: Album) : Item {
        override val artwork get() = album.artwork
        override val title get() = album.title
    }

    data class ArtistItem(val artist: Artist) : Item {
        override val artwork get() = artist.artwork
        override val title get() = artist.name
    }

    data class PlaylistItem(val playlist: Playlist) : Item {
        override val artwork get() = playlist.artwork
        override val title get() = playlist.title
    }

    data class PodcastItem(val podcast: Podcast) : Item {
        override val artwork get() = podcast.artwork
        override val title get() = podcast.title
    }

    data class Episode(val track: Track) : Item {
        override val artwork get() = track.artwork
        override val title get() = track.title
    }
}

/** How the core writes an item: a kind, and the one field that kind names. */
@Serializable
internal data class WireItem(
    val kind: String = "",
    val track: Track? = null,
    val album: Album? = null,
    val artist: Artist? = null,
    val playlist: Playlist? = null,
    val podcast: Podcast? = null,
    val episode: Track? = null,
) {
    /** Null for a kind this client does not know: one odd item must not cost its page. */
    fun item(): Item? = when (kind) {
        "track" -> track?.let(Item::Song)
        "album" -> album?.let(Item::AlbumItem)
        "artist" -> artist?.let(Item::ArtistItem)
        "playlist" -> playlist?.let(Item::PlaylistItem)
        "podcast" -> podcast?.let(Item::PodcastItem)
        "episode" -> episode?.let(Item::Episode)
        else -> null
    }
}

@Serializable
data class MoodChip(val id: String = "", val params: String = "", val title: String = "", val color: String = "")

@Serializable
data class HomeChip(val title: String = "", val params: String = "", val selected: Boolean = false)

@Serializable
data class BrowsePage(
    val title: String = "",
    val shelves: List<Shelf> = emptyList(),
    val moods: List<MoodChip> = emptyList(),
    val chips: List<HomeChip> = emptyList(),
    val continuation: String = "",
)

@Serializable
data class SearchHistoryEntry(val query: String = "", val token: String = "")

@Serializable
data class SearchResults(
    val query: String = "",
    @Serializable(with = ItemSerializer::class) val topResult: Item? = null,
    @Serializable(with = ItemListSerializer::class) val topResultItems: List<Item> = emptyList(),
    val shelves: List<Shelf> = emptyList(),
)

enum class SearchFilter(val label: String, val wire: String) {
    All("All", ""),
    Songs("Songs", "songs"),
    Albums("Albums", "albums"),
    Artists("Artists", "artists"),
    Playlists("Playlists", "playlists"),
    Podcasts("Podcasts", "podcasts"),
    Episodes("Episodes", "episodes"),
}

@Serializable
enum class LibraryKind {
    @SerialName("playlist") Playlist,
    @SerialName("album") Album,
    @SerialName("artist") Artist,
    @SerialName("podcast") Podcast;

    val wire: String get() = name.lowercase()
}

@Serializable
data class LibraryItem(
    val id: String = "",
    val kind: LibraryKind = LibraryKind.Playlist,
    val title: String = "",
    val subtitle: String = "",
    val artwork: List<Artwork> = emptyList(),
    val pinned: Boolean = false,
    val folderId: String = "",
    val addedAt: String? = null,
    val lastPlayedAt: String? = null,
)

@Serializable
data class Folder(val id: String = "", val name: String = "")

/** One word of a timed line and when it is sung; its text keeps the space after it. */
@Serializable
data class LyricWord(val atMs: Long = 0, val text: String = "")

/** One line of lyrics; [words] time it word by word, when the source does. */
@Serializable
data class LyricLine(val atMs: Long = 0, val text: String = "", val words: List<LyricWord> = emptyList())

@Serializable
data class Lyrics(
    val lines: List<LyricLine> = emptyList(),
    val synced: Boolean = false,
    val plain: String = "",
    val source: String = "",
) {
    /** Lyrics that came as one text, cut into lines to show. */
    fun withLines(): Lyrics =
        if (lines.isNotEmpty()) this
        else copy(lines = plain.lines().map { LyricLine(0, it) }, synced = false)

    /** The line being sung at [positionMs]: the last one that has started. */
    fun lineAt(positionMs: Long): Int? {
        if (!synced) return null
        val started = lines.indexOfLast { it.atMs <= positionMs }
        return started.takeIf { it >= 0 }
    }
}

@Serializable
data class Mix(
    val id: String = "",
    val title: String = "",
    val description: String = "",
    val tracks: List<Track> = emptyList(),
)

@Serializable
data class CacheUsage(val bytes: Long = 0, val tracks: Int = 0)

@Serializable
data class Account(val name: String = "", val handle: String = "", val avatarUrl: String = "")

@Serializable
data class RemoteQueue(val tracks: List<Track> = emptyList(), val index: Int = 0, val title: String = "")

// ---------- the session ----------

@Serializable
enum class PlayState {
    @SerialName("idle") Idle,
    @SerialName("loading") Loading,
    @SerialName("playing") Playing,
    @SerialName("paused") Paused,
    @SerialName("stalled") Stalled,
}

@Serializable
enum class Repeat {
    @SerialName("off") Off,
    @SerialName("one") One,
    @SerialName("all") All,
}

@Serializable
data class Queue(val items: List<Track> = emptyList(), val index: Int = 0, val origin: String = "")

@Serializable
data class SessionState(
    val version: Long = 0,
    val queue: Queue = Queue(),
    val state: PlayState = PlayState.Idle,
    val repeat: Repeat = Repeat.Off,
    val shuffle: Boolean = false,
    val volume: Float = 1f,
    val positionMs: Long = 0,
) {
    val current: Track? get() = queue.items.getOrNull(queue.index)
}

/** How one track gives way to the next. The core writes it with Go's names. */
@Serializable
data class Transition(@SerialName("Kind") val kind: String = "cut", @SerialName("Ms") val ms: Long = 0) {
    val crossfadeMs: Long get() = if (kind == "crossfade") ms else 0
    /** Whether the engine runs into the next track by itself at the end. */
    val runsOn: Boolean get() = kind != "cut"
}

/** What the engine should be doing, with Go's field names. */
@Serializable
data class Target(
    @SerialName("Epoch") val epoch: Long = 0,
    @SerialName("VideoID") val videoId: String = "",
    @SerialName("StartAtMs") val startAtMs: Long = 0,
    @SerialName("Playing") val playing: Boolean = false,
    @SerialName("PreloadVideoID") val preloadVideoId: String = "",
    @SerialName("Volume") val volume: Float = 1f,
    @SerialName("Transition") val transition: Transition = Transition(),
    @SerialName("UserChange") val userChange: Boolean = false,
)

@Serializable
data class RoomPlayback(val entry: String = "", val ended: Boolean = false, val durationMs: Long = 0)

/** The core's whole view of playback, sent on every change. */
@Serializable
data class Projection(
    val state: SessionState = SessionState(),
    val target: Target = Target(),
    val followingRoom: Boolean = false,
    val room: RoomPlayback? = null,
    val offline: Boolean = false,
)

package dev.darksavci.spotified.api

import kotlinx.serialization.Serializable

/*
 * Your listening: what the core counts from the plays recorded here. None of
 * it goes to YouTube. Mirrors crates/client/src/models/stats.rs.
 */

@Serializable
data class TrackStat(
    val trackId: String = "",
    val title: String = "",
    val artist: String = "",
    val artistId: String = "",
    val plays: Int = 0,
    val totalMs: Long = 0,
    val artwork: String = "",
) {
    fun track() = Track(
        id = trackId,
        title = title,
        artists = listOf(ArtistRef(channelId(artistId) ?: "", artist)),
        artwork = cover(artwork),
        playCount = plays.toString(),
    )
}

@Serializable
data class ArtistStat(
    val artistId: String = "",
    val artist: String = "",
    val plays: Int = 0,
    val distinctTracks: Int = 0,
    val totalMs: Long = 0,
    val artwork: String = "",
)

@Serializable
data class AlbumStat(
    val key: String = "",
    val albumId: String = "",
    val album: String = "",
    val artist: String = "",
    val artistId: String = "",
    val plays: Int = 0,
    val distinctTracks: Int = 0,
    val totalMs: Long = 0,
    val artwork: String = "",
)

@Serializable
data class Summary(
    val plays: Int = 0,
    val totalMs: Long = 0,
    val distinctTracks: Int = 0,
    val distinctArtists: Int = 0,
    val distinctAlbums: Int = 0,
)

data class Stats(
    val summary: Summary,
    val tracks: List<TrackStat>,
    val artists: List<ArtistStat>,
    val albums: List<AlbumStat>,
    val onRepeat: List<TrackStat>,
)

@Serializable
data class MonthPlays(val month: String = "", val plays: Int = 0, val totalMs: Long = 0)

@Serializable
data class StatDetail(
    val kind: String = "track",
    val id: String = "",
    val name: String = "",
    val artist: String = "",
    val artistId: String = "",
    val albumId: String = "",
    val artwork: String = "",
    val plays: Int = 0,
    val plays30d: Int = 0,
    val totalMs: Long = 0,
    val distinctTracks: Int = 0,
    val firstPlayedAt: String? = null,
    val lastPlayedAt: String? = null,
    val rank: Int = 0,
    val months: List<MonthPlays> = emptyList(),
    val topTracks: List<TrackStat> = emptyList(),
)

@Serializable
data class LookupResults(
    val tracks: List<TrackStat>? = null,
    val artists: List<ArtistStat>? = null,
    val albums: List<AlbumStat>? = null,
)

/** A picture the stats routes give as a bare address. */
fun cover(url: String): List<Artwork> = if (url.isEmpty()) emptyList() else listOf(Artwork(url, 226, 226))

/** The id when it is a channel id (UC…), which an artist page can be opened by. */
fun channelId(id: String): String? {
    val rest = id.removePrefix("UC")
    if (rest == id || rest.length < 10) return null
    return id.takeIf { rest.all { it.isLetterOrDigit() || it == '_' || it == '-' } }
}

package dev.darksavci.spotified.data

import android.content.ContentUris
import android.content.Context
import android.net.Uri
import android.provider.MediaStore
import dev.darksavci.spotified.api.AlbumRef
import dev.darksavci.spotified.api.ArtistRef
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Track
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The music files on the phone, as tracks that queue beside YouTube Music's.
 * Their ids start "local-": the engine plays them from the phone itself, and
 * the core, which cannot resolve them, only keeps them in the queue.
 */
object LocalMusic {
    const val PREFIX = "local-"

    fun isLocal(id: String) = id.startsWith(PREFIX)

    /** The file a local track's id names. */
    fun uriOf(id: String): Uri? =
        id.removePrefix(PREFIX).toLongOrNull()?.let { ContentUris.withAppendedId(MediaStore.Audio.Media.EXTERNAL_CONTENT_URI, it) }

    suspend fun scan(context: Context): List<Track> = withContext(Dispatchers.IO) {
        val columns = arrayOf(
            MediaStore.Audio.Media._ID, MediaStore.Audio.Media.TITLE, MediaStore.Audio.Media.ARTIST,
            MediaStore.Audio.Media.ALBUM, MediaStore.Audio.Media.ALBUM_ID, MediaStore.Audio.Media.DURATION,
        )
        val tracks = mutableListOf<Track>()
        context.contentResolver.query(
            MediaStore.Audio.Media.EXTERNAL_CONTENT_URI, columns, "${MediaStore.Audio.Media.IS_MUSIC} != 0", null,
            "${MediaStore.Audio.Media.TITLE} COLLATE NOCASE",
        )?.use { c ->
            while (c.moveToNext()) {
                val albumId = c.getLong(4)
                val art = ContentUris.withAppendedId(Uri.parse("content://media/external/audio/albumart"), albumId).toString()
                tracks += Track(
                    id = PREFIX + c.getLong(0),
                    title = c.getString(1).orEmpty(),
                    artists = listOf(ArtistRef("", c.getString(2)?.takeIf { it != "<unknown>" }.orEmpty())),
                    album = AlbumRef("", c.getString(3).orEmpty()),
                    durationMs = c.getLong(5),
                    artwork = listOf(Artwork(art, 300, 300)),
                )
            }
        }
        tracks
    }
}

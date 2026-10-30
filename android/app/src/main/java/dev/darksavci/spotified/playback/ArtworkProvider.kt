package dev.darksavci.spotified.playback

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.util.Base64
import dev.darksavci.spotified.data.Logs
import dev.darksavci.spotified.graph
import okhttp3.Request
import java.io.File
import java.io.FileNotFoundException
import java.security.MessageDigest

/**
 * Artwork for Android Auto and the system's media surfaces, as content://
 * addresses: the car's head unit cannot fetch the web itself. A picture is
 * fetched once into the cache and served from there.
 *
 * Only Google's image hosts are fetched, so no other app can use this to make
 * the phone ask the web for something on its behalf.
 */
class ArtworkProvider : ContentProvider() {
    override fun onCreate() = true

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        val encoded = uri.lastPathSegment ?: throw FileNotFoundException()
        val source = runCatching { String(Base64.decode(encoded, Base64.URL_SAFE or Base64.NO_WRAP)) }
            .getOrNull() ?: throw FileNotFoundException()
        val host = Uri.parse(source).host.orEmpty()
        if (!source.startsWith("https://") || ALLOWED.none { host == it || host.endsWith(".$it") }) {
            throw FileNotFoundException("not an artwork host: $host")
        }
        val context = context ?: throw FileNotFoundException()
        val dir = File(context.cacheDir, "artwork").apply { mkdirs() }
        val file = File(dir, sha(source))
        if (!file.exists()) {
            val request = Request.Builder().url(source).build()
            context.graph.http.newCall(request).execute().use { response ->
                if (!response.isSuccessful) {
                    Logs.w(TAG, "artwork from $host: HTTP ${response.code}")
                    throw FileNotFoundException("HTTP ${response.code}")
                }
                val tmp = File(dir, file.name + ".part")
                tmp.outputStream().use { response.body.byteStream().copyTo(it) }
                tmp.renameTo(file)
            }
            prune(dir)
        }
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun getType(uri: Uri) = "image/jpeg"
    override fun query(uri: Uri, p: Array<String>?, s: String?, a: Array<String>?, o: String?): Cursor? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, s: String?, a: Array<String>?) = 0
    override fun update(uri: Uri, v: ContentValues?, s: String?, a: Array<String>?) = 0

    private fun sha(text: String): String =
        MessageDigest.getInstance("SHA-256").digest(text.toByteArray()).take(12).joinToString("") { "%02x".format(it) }

    /** Keeps the newest few hundred pictures. */
    private fun prune(dir: File) {
        val files = dir.listFiles()?.sortedByDescending { it.lastModified() } ?: return
        files.drop(KEEP).forEach { it.delete() }
    }

    companion object {
        const val AUTHORITY = "dev.darksavci.spotified.artwork"
        private const val KEEP = 400
        private const val TAG = "Artwork"
        // gstatic.com holds the pictures of YouTube Music's own lists, such as Liked Music.
        private val ALLOWED = listOf("googleusercontent.com", "ytimg.com", "ggpht.com", "gstatic.com")

        /** The content:// address that serves [url], or null for none. */
        fun uriFor(url: String?): Uri? {
            if (url.isNullOrEmpty()) return null
            val encoded = Base64.encodeToString(url.toByteArray(), Base64.URL_SAFE or Base64.NO_WRAP)
            return Uri.Builder().scheme("content").authority(AUTHORITY).appendPath(encoded).build()
        }
    }
}

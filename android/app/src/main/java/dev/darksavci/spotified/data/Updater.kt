package dev.darksavci.spotified.data

import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.content.FileProvider
import dev.darksavci.spotified.BuildConfig
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.security.MessageDigest

/**
 * Updates from the project's GitHub releases, as the desktop's updater does
 * (crates/app/src/update.rs): the newest release's APK for this phone,
 * checked against the release's SHA256SUMS.txt before Android is asked to
 * install it.
 */
object Updater {
    private const val RELEASES = "https://api.github.com/repos/DarkSavci/youtube-music-spotified-native/releases/latest"

    suspend fun check(context: Context, http: OkHttpClient, say: (String) -> Unit) = withContext(Dispatchers.IO) {
        val release = http.newCall(Request.Builder().url(RELEASES).header("Accept", "application/vnd.github+json").build())
            .execute().use { r ->
                if (r.code == 404) return@withContext say("There are no releases yet.")
                if (!r.isSuccessful) error("GitHub answered ${r.code}")
                Json.parseToJsonElement(r.body.string()) as JsonObject
            }
        val tag = (release["tag_name"] as? JsonPrimitive)?.content.orEmpty().removePrefix("v")
        if (!newer(tag, BuildConfig.VERSION_NAME)) return@withContext say("This is the newest version (${BuildConfig.VERSION_NAME}).")
        val assets = (release["assets"] as? JsonArray).orEmpty().mapNotNull { it as? JsonObject }
        fun asset(name: (String) -> Boolean) = assets.firstOrNull { name((it["name"] as? JsonPrimitive)?.content.orEmpty()) }
        val abi = Build.SUPPORTED_ABIS.firstOrNull().orEmpty()
        val apk = asset { it.endsWith(".apk") && abi in it } ?: asset { it.endsWith(".apk") }
            ?: return@withContext say("Version $tag has no APK for this phone.")
        val sums = asset { it == "SHA256SUMS.txt" } ?: return@withContext say("Version $tag has no checksums; it was not installed.")
        val apkName = (apk["name"] as JsonPrimitive).content
        say("Downloading version $tag…")
        val expected = text(http, url(sums)).lines().firstOrNull { it.trim().endsWith(apkName) }?.trim()?.substringBefore(' ')
            ?: return@withContext say("The checksums do not list $apkName; it was not installed.")
        val dir = File(context.cacheDir, "updates").apply { mkdirs() }
        val file = File(dir, "update.apk")
        http.newCall(Request.Builder().url(url(apk)).build()).execute().use { r ->
            if (!r.isSuccessful) error("GitHub answered ${r.code}")
            file.outputStream().use { r.body.byteStream().copyTo(it) }
        }
        val actual = MessageDigest.getInstance("SHA-256").digest(file.readBytes()).joinToString("") { "%02x".format(it) }
        if (!actual.equals(expected, ignoreCase = true)) {
            file.delete()
            return@withContext say("The download does not match its checksum; it was not installed.")
        }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
        val install = Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive")
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(install)
    }

    private fun url(asset: JsonObject) = (asset["browser_download_url"] as JsonPrimitive).content

    private fun text(http: OkHttpClient, url: String) =
        http.newCall(Request.Builder().url(url).build()).execute().use { it.body.string() }

    /** Whether version [a] is newer than [b], comparing their numbers in order. */
    fun newer(a: String, b: String): Boolean {
        val x = a.split('.', '-').map { it.toIntOrNull() ?: 0 }
        val y = b.split('.', '-').map { it.toIntOrNull() ?: 0 }
        for (i in 0 until maxOf(x.size, y.size)) {
            val d = x.getOrElse(i) { 0 } - y.getOrElse(i) { 0 }
            if (d != 0) return d > 0
        }
        return false
    }
}

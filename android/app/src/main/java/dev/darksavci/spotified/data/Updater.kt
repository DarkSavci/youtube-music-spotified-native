package dev.darksavci.spotified.data

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.os.Build
import dev.darksavci.spotified.BuildConfig
import dev.darksavci.spotified.graph
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
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
    private val RELEASES = BuildConfig.RELEASES_URL

    suspend fun check(
        context: Context, http: OkHttpClient, say: (String) -> Unit, current: String = BuildConfig.VERSION_NAME,
        install: (File) -> Unit = { install(context, it) },
    ) = withContext(Dispatchers.IO) {
        val release = http.newCall(Request.Builder().url(RELEASES).header("Accept", "application/vnd.github+json").build())
            .execute().use { r ->
                if (r.code == 404) return@withContext say("There are no releases yet.")
                if (!r.isSuccessful) error("GitHub answered ${r.code}")
                phoneRelease(Json.parseToJsonElement(r.body.string()))
            } ?: return@withContext say("There are no releases for the phone yet.")
        val tag = (release["tag_name"] as? JsonPrimitive)?.content.orEmpty().removePrefix("android-").removePrefix("v")
        if (!newer(tag, current)) return@withContext say("This is the newest version ($current).")
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
        install(file)
    }

    /**
     * Hands [apk] to Android's installer as a session, so how it ends comes
     * back to [Installed]: Android asks the listener to confirm, and a
     * refusal (Play Protect's, say) is said rather than lost.
     */
    fun install(context: Context, apk: File) {
        val installer = context.packageManager.packageInstaller
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
            .apply { setAppPackageName(context.packageName) }
        val id = installer.createSession(params)
        installer.openSession(id).use { session ->
            session.openWrite("update.apk", 0, apk.length()).use { out ->
                apk.inputStream().use { it.copyTo(out) }
                session.fsync(out)
            }
            val done = PendingIntent.getBroadcast(
                context, id, Intent(context, Installed::class.java),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE,
            )
            session.commit(done.intentSender)
        }
        apk.delete()
    }

    /** How an install ended: Android's confirmation shown, or why it was refused. */
    class Installed : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)
            val say = context.graph.playback::say
            when (status) {
                PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                    @Suppress("DEPRECATION")
                    val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT) ?: return
                    context.startActivity(confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                }
                PackageInstaller.STATUS_SUCCESS -> Unit
                PackageInstaller.STATUS_FAILURE_ABORTED -> say("The update was cancelled.")
                PackageInstaller.STATUS_FAILURE_BLOCKED ->
                    say("Android blocked the update. Install it from the project's GitHub releases instead.")
                PackageInstaller.STATUS_FAILURE_STORAGE -> say("There is not enough space for the update.")
                else -> say("The update was not installed: ${intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE) ?: "Android refused it"}.")
            }
        }
    }

    /**
     * The newest published release for the phone. The desktop's releases share
     * the repository and its "latest", so the list is read for the newest
     * android-v tag; a single release object (a test's) is taken as it is.
     */
    private fun phoneRelease(answer: JsonElement): JsonObject? = when (answer) {
        is JsonObject -> answer
        is JsonArray -> answer.mapNotNull { it as? JsonObject }.firstOrNull { r ->
            val tag = (r["tag_name"] as? JsonPrimitive)?.content.orEmpty()
            val flag = { key: String -> (r[key] as? JsonPrimitive)?.content == "true" }
            tag.startsWith("android-v") && !flag("draft") && !flag("prerelease")
        }
        else -> null
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

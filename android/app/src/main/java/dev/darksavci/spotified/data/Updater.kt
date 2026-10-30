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
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
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
    private val RELEASES = BuildConfig.RELEASES_URL

    /** How often the quiet check at launch asks GitHub. */
    private const val QUIET_EVERY_MS = 24 * 60 * 60 * 1000L

    /** Where an update is, for Settings to show and the badge to follow. */
    sealed interface Status {
        object Idle : Status
        object Checking : Status
        data class Current(val version: String) : Status
        data class Available(val version: String) : Status
        /** Coming down; [fraction] once its size is known. */
        data class Downloading(val version: String, val fraction: Float?) : Status
        data class Installing(val version: String) : Status
        data class Failed(val message: String) : Status
    }

    private val _status = MutableStateFlow<Status>(Status.Idle)
    val status: StateFlow<Status> = _status

    /** Whether a tap on the row is free to start something. */
    val busy: Boolean get() = _status.value.let { it is Status.Checking || it is Status.Downloading || it is Status.Installing }

    /** The newest release as far as this phone cares: its version and the files to fetch. */
    private class Latest(val version: String, val apkName: String?, val apkUrl: String?, val sumsUrl: String?)

    private fun latest(http: OkHttpClient): Latest? {
        val release = http.newCall(Request.Builder().url(RELEASES).header("Accept", "application/vnd.github+json").build())
            .execute().use { r ->
                if (r.code == 404) return null
                if (!r.isSuccessful) error("GitHub answered ${r.code}")
                Json.parseToJsonElement(r.body.string()) as? JsonObject
            } ?: error("GitHub's answer was not a release")
        val assets = (release["assets"] as? JsonArray).orEmpty().mapNotNull { it as? JsonObject }
        val names = assets.map { (it["name"] as? JsonPrimitive)?.content.orEmpty() }
        fun address(name: String?) = assets.firstOrNull { (it["name"] as? JsonPrimitive)?.content == name }?.let(::url)
        val apk = pickApk(names, Build.SUPPORTED_ABIS.firstOrNull().orEmpty())
        return Latest(
            (release["tag_name"] as? JsonPrimitive)?.content.orEmpty().removePrefix("v"),
            apk, address(apk), address("SHA256SUMS.txt"),
        )
    }

    /** The APK for this phone, or the one that holds them all. */
    internal fun pickApk(names: List<String>, abi: String): String? =
        names.firstOrNull { it.endsWith(".apk") && abi.isNotEmpty() && abi in it } ?: names.firstOrNull { it.endsWith("-universal.apk") }

    /** What SHA256SUMS.txt says of [apk]. */
    internal fun expectedSum(sums: String, apk: String): String? =
        sums.lines().firstOrNull { it.trim().endsWith(apk) }?.trim()?.substringBefore(' ')

    /** Asks GitHub and says what it found, downloading nothing. */
    suspend fun look(http: OkHttpClient, current: String = BuildConfig.VERSION_NAME) = withContext(Dispatchers.IO) {
        _status.value = Status.Checking
        _status.value = runCatching { latest(http) }.fold(
            { found ->
                when {
                    found == null -> Status.Failed("There are no releases yet.")
                    newer(found.version, current) -> Status.Available(found.version)
                    else -> Status.Current(current)
                }
            },
            { Status.Failed("Couldn't check for updates. Check your connection and try again.") },
        )
    }

    /** The check at launch: at most once a day, and it says nothing but the badge unless there is an update. */
    suspend fun lookQuietly(context: Context, http: OkHttpClient, now: Long = System.currentTimeMillis()) = withContext(Dispatchers.IO) {
        val stamp = File(context.filesDir, "update-check.txt")
        val last = stamp.takeIf { it.isFile }?.readText()?.trim()?.toLongOrNull() ?: 0L
        val found = (_status.value as? Status.Available)?.version
        if (found != null || busy || !dueForLook(last, now)) return@withContext
        runCatching { stamp.writeText(now.toString()) }
        val answer = runCatching { latest(http) }.onFailure { Logs.i(TAG, "quiet update check: ${it.message}") }.getOrNull()
        if (answer != null && newer(answer.version, BuildConfig.VERSION_NAME)) _status.value = Status.Available(answer.version)
    }

    internal fun dueForLook(last: Long, now: Long) = now - last >= QUIET_EVERY_MS || now < last

    /** Finds the newest release and, if it is newer, downloads, checks and installs it. Each outcome is said as well as shown. */
    suspend fun check(
        context: Context, http: OkHttpClient, say: (String) -> Unit, current: String = BuildConfig.VERSION_NAME,
        install: (File) -> Unit = { install(context, it) },
    ) = withContext(Dispatchers.IO) {
        fun ended(message: String) {
            _status.value = Status.Failed(message)
            say(message)
        }
        _status.value = Status.Checking
        val release = runCatching { latest(http) }.getOrElse {
            _status.value = Status.Failed("Couldn't check for updates. Check your connection and try again.")
            throw it
        } ?: return@withContext ended("There are no releases yet.")
        val tag = release.version
        if (!newer(tag, current)) {
            _status.value = Status.Current(current)
            return@withContext say("This is the newest version ($current).")
        }
        val apkName = release.apkName ?: return@withContext ended("Version $tag has no APK for this phone.")
        val apkUrl = release.apkUrl ?: return@withContext ended("Version $tag has no APK for this phone.")
        val sumsUrl = release.sumsUrl ?: return@withContext ended("Version $tag has no checksums; it was not installed.")
        say("Downloading version $tag…")
        _status.value = Status.Downloading(tag, null)
        val digest = MessageDigest.getInstance("SHA-256")
        val file = File(File(context.cacheDir, "updates").apply { mkdirs() }, "update.apk")
        try {
            val expected = expectedSum(text(http, sumsUrl), apkName)
                ?: return@withContext ended("The checksums do not list $apkName; it was not installed.")
            http.newCall(Request.Builder().url(apkUrl).build()).execute().use { r ->
                if (!r.isSuccessful) error("GitHub answered ${r.code}")
                val size = r.body.contentLength()
                var have = 0L
                var told = -1
                file.outputStream().use { out ->
                    val input = r.body.byteStream()
                    val buffer = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buffer)
                        if (n < 0) break
                        out.write(buffer, 0, n)
                        digest.update(buffer, 0, n)
                        have += n
                        val percent = if (size > 0) (have * 100 / size).toInt() else -1
                        if (percent != told) {
                            told = percent
                            _status.value = Status.Downloading(tag, if (size > 0) have.toFloat() / size else null)
                        }
                    }
                }
            }
            val actual = digest.digest().joinToString("") { "%02x".format(java.util.Locale.ROOT, it) }
            if (!actual.equals(expected, ignoreCase = true)) {
                file.delete()
                return@withContext ended("The download does not match its checksum; it was not installed.")
            }
        } catch (e: Exception) {
            file.delete()
            _status.value = Status.Failed("The download stopped. Try again when your connection is better.")
            throw e
        }
        _status.value = Status.Installing(tag)
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
            val told = context.graph.playback::say
            val say = { message: String ->
                _status.value = Status.Failed(message)
                told(message)
            }
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

    private const val TAG = "Updater"

    private fun url(asset: JsonObject) = (asset["browser_download_url"] as JsonPrimitive).content

    private fun text(http: OkHttpClient, url: String) =
        http.newCall(Request.Builder().url(url).build()).execute().use { it.body.string() }

    /**
     * Whether version [a] is newer than [b], comparing their numbers in order;
     * a pre-release (0.5.0-beta.2) comes before its release (0.5.0).
     */
    fun newer(a: String, b: String): Boolean {
        fun parts(v: String) = v.substringBefore('-').split('.').map { it.toIntOrNull() ?: 0 }
        fun pre(v: String) = if ('-' in v) v.substringAfterLast('.').toIntOrNull() ?: 0 else Int.MAX_VALUE
        val x = parts(a)
        val y = parts(b)
        for (i in 0 until maxOf(x.size, y.size)) {
            val d = x.getOrElse(i) { 0 } - y.getOrElse(i) { 0 }
            if (d != 0) return d > 0
        }
        return pre(a) > pre(b)
    }
}

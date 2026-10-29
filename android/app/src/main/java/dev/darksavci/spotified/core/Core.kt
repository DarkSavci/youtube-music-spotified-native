package dev.darksavci.spotified.core

import dev.darksavci.spotified.BuildConfig
import android.content.Context
import android.util.Log
import com.chaquo.python.PyException
import com.chaquo.python.PyObject
import com.chaquo.python.Python
import com.chaquo.python.android.AndroidPlatform
import dev.darksavci.spotified.core.mobile.Mobile
import dev.darksavci.spotified.core.mobile.YtdlpRunner
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.io.File

/**
 * The Go core, run in this process on the loopback address. It lives as long
 * as the process: the activity and the playback service both use it, and
 * whichever asks first starts it.
 */
class Core(context: Context) {
    private val app = context.applicationContext
    private val lock = Mutex()

    /** The accounts on the phone; the core runs in the folder of the one in use. */
    val accounts = Accounts(app)

    /** Where the core answers, and the token every request must carry. */
    data class Running(val address: String, val token: String)

    @Volatile
    private var running: Running? = null

    /** The running core, started if it is not. */
    suspend fun running(): Running = running ?: lock.withLock {
        running ?: withContext(Dispatchers.IO) { start() }.also { running = it }
    }

    /** Stops the core and starts it again, as a sign-in needs: yt-dlp's cookie file is written at start. */
    suspend fun restart(): Running = lock.withLock {
        withContext(Dispatchers.IO) {
            running = null
            Mobile.stop()
            start().also { running = it }
        }
    }

    /** The running core's address, or null before it has started. */
    fun currentAddress(): String? = running?.address

    /** The running core's token, or null before it has started; for the request interceptor. */
    fun currentToken(): String? = running?.token

    /** Stops the core once it has saved where the listener is. Blocks. */
    fun stop() {
        running = null
        Mobile.stop()
    }

    private fun start(): Running {
        val cfg = Mobile.newConfig()
        cfg.dataDir = accounts.currentDir().path
        cfg.cacheDir = app.cacheDir.path
        // yt-dlp, and the core's own resolver, run qjs as a program, from
        // where Android put it.
        cfg.jsRuntimes = "quickjs:${app.applicationInfo.nativeLibraryDir}/libqjs.so"
        cfg.verbose = BuildConfig.DEBUG
        Mobile.setYtdlp(PythonYtdlp(app))
        val started = System.currentTimeMillis()
        val address = Mobile.start(cfg)
        Log.i(TAG, "core on $address in ${System.currentTimeMillis() - started} ms")
        return Running(address, Mobile.token())
    }

    companion object {
        const val TAG = "Core"
        val tokenHeader: String get() = Mobile.TokenHeader
    }
}

/**
 * yt-dlp as the core asks for it, run by Chaquopy's Python.
 *
 * Started on the first call, not with the core: a Premium account is served
 * by the core's own resolver, and Python with yt-dlp loaded costs seconds and
 * tens of megabytes a listener may never need.
 */
private class PythonYtdlp(private val app: Context) : YtdlpRunner {
    private val bridge: PyObject by lazy {
        if (!Python.isStarted()) Python.start(AndroidPlatform(app))
        Python.getInstance().getModule("ytdlp_bridge").also {
            it.callAttr("configure", app.cacheDir.path, File(app.cacheDir, "yt-dlp").path, false)
        }
    }

    override fun run(args: String): String =
        try {
            bridge.callAttr("run", args).toString()
        } catch (e: PyException) {
            // The message is yt-dlp's own, which the core reads to tell a
            // rate limit from a track that cannot be played.
            throw Exception(e.message)
        }
}

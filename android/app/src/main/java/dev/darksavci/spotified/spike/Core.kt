package dev.darksavci.spotified.spike

import android.content.Context
import com.chaquo.python.PyException
import com.chaquo.python.PyObject
import com.chaquo.python.Python
import com.chaquo.python.android.AndroidPlatform
import dev.darksavci.spotified.core.mobile.Mobile
import dev.darksavci.spotified.core.mobile.YtdlpRunner

/** The Go core, run in this process on 127.0.0.1. */
object Core {
    @Volatile
    private var address: String? = null

    /**
     * Starts the core with yt-dlp under Python and returns its address, or
     * the running core's: it lives as long as the process, not an activity.
     * Blocks.
     */
    @Synchronized
    fun start(context: Context): String {
        address?.let { return it }
        val cfg = Mobile.newConfig()
        cfg.dataDir = context.filesDir.path
        cfg.cacheDir = context.cacheDir.path
        // yt-dlp runs qjs as a program, from where Android put it.
        cfg.jsRuntimes = "quickjs:${context.applicationInfo.nativeLibraryDir}/libqjs.so"
        Mobile.setYtdlp(PythonYtdlp(context))
        return Mobile.start(cfg).also { address = it }
    }

    /** The header and value every request to the core must carry. */
    fun auth(): Pair<String, String> = Mobile.TokenHeader to Mobile.token()

    /** Stops the core once it has saved where the listener is. Blocks. */
    @Synchronized
    fun stop() {
        address = null
        Mobile.stop()
    }
}

/** Set before the core starts: yt-dlp then logs everything it does. */
@Volatile
var VERBOSE = false

/**
 * yt-dlp as the core asks for it, run by Chaquopy's Python.
 *
 * Started on the first call, not with the core: a Premium account is served
 * by the core's own resolver, and Python with yt-dlp loaded costs seconds
 * and tens of megabytes a listener may never need.
 */
private class PythonYtdlp(context: Context) : YtdlpRunner {
    private val app = context.applicationContext

    private val bridge: PyObject by lazy {
        if (!Python.isStarted()) Python.start(AndroidPlatform(app))
        Python.getInstance().getModule("ytdlp_bridge").also {
            it.callAttr(
                "configure",
                app.cacheDir.path,
                java.io.File(app.cacheDir, "yt-dlp").path,
                VERBOSE,
            )
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

package dev.darksavci.spotified.data

import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileOutputStream
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.Callable
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The app's log: Android's, and a small file of the same lines that outlasts
 * it. Logcat keeps minutes of a busy phone, too little for a problem noticed
 * after the fact. The file stays on the phone until the listener sends it.
 */
object Logs {
    private const val CORE_TAG = "Core"
    private const val SHARED_NAME = "spotified-log.txt"

    @Volatile
    private var file: RotatingLog? = null
    private val writer = Executors.newSingleThreadExecutor { job -> Thread(job, "log-file").apply { isDaemon = true } }
    private val stamp = SimpleDateFormat("MM-dd HH:mm:ss.SSS", Locale.US)

    /** Starts the file. Lines from before this reach only Android's log. */
    fun install(context: Context) {
        startFile(File(context.filesDir, "logs"))
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            e("Crash", "uncaught on ${thread.name}", error)
            drain()
            previous?.uncaughtException(thread, error)
        }
    }

    internal fun startFile(dir: File) {
        file = RotatingLog(dir)
    }

    fun d(tag: String, message: String) = log(Log.DEBUG, tag, message, null)
    fun i(tag: String, message: String) = log(Log.INFO, tag, message, null)
    fun w(tag: String, message: String, error: Throwable? = null) = log(Log.WARN, tag, message, error)
    fun e(tag: String, message: String, error: Throwable? = null) = log(Log.ERROR, tag, message, error)

    /** A line from the Go core, at the Android level it was mapped to there. */
    fun fromCore(priority: Int, line: String) = log(priority, CORE_TAG, line, null)

    private fun log(priority: Int, tag: String, message: String, error: Throwable?) {
        // Scrubbed on the way in: cookies and signed addresses are never written
        // down, so there are none to leak later.
        val text = Redact.text(if (error == null) message else "$message\n${Log.getStackTraceString(error)}")
        Log.println(priority, tag, text)
        val out = file ?: return
        val at = System.currentTimeMillis()
        writer.execute {
            runCatching { out.append("${stamp.format(Date(at))} ${LEVELS.getOrElse(priority) { '?' }} $tag: $text") }
        }
    }

    /** Waits a moment for the lines on their way to reach the file. */
    private fun drain() {
        runCatching { writer.submit { }.get(1, TimeUnit.SECONDS) }
    }

    /** Everything in the file, oldest first. */
    suspend fun text(): String = withContext(Dispatchers.IO) {
        // On the writer's thread, so a rotation cannot happen under the read.
        writer.submit(Callable { file?.readAll().orEmpty() }).get()
    }

    /**
     * An intent that sends the log as a text file, for a Logs or Problem
     * report button: `context.startActivity(Logs.shareIntent(context))`. The
     * file goes where the listener chooses and nowhere else.
     */
    suspend fun shareIntent(context: Context): Intent {
        val body = text()
        val shared = withContext(Dispatchers.IO) {
            val dir = File(context.cacheDir, "share").apply { mkdirs() }
            File(dir, SHARED_NAME).apply { writeText(body) }
        }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", shared)
        val send = Intent(Intent.ACTION_SEND).setType("text/plain")
            .putExtra(Intent.EXTRA_STREAM, uri)
            .putExtra(Intent.EXTRA_SUBJECT, "Youtube Music Spotified log")
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        return Intent.createChooser(send, "Send the log").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }

    private val LEVELS = mapOf(
        Log.VERBOSE to 'V', Log.DEBUG to 'D', Log.INFO to 'I', Log.WARN to 'W', Log.ERROR to 'E', Log.ASSERT to 'A',
    )
}

/**
 * Lines appended to a file that is set aside when full: "app.log", then
 * "app.log.1" and so on, oldest last, with those past [keep] deleted. Not safe
 * for concurrent use; [Logs] writes from one thread.
 */
class RotatingLog(
    private val dir: File,
    private val maxBytes: Long = 512 * 1024,
    private val keep: Int = 2,
    private val maxLineChars: Int = 8192,
) {
    private val current = File(dir, NAME)
    private var out: FileOutputStream? = null
    private var size = 0L

    fun append(line: String) {
        val bytes = (line.take(maxLineChars) + "\n").toByteArray()
        val stream = out ?: open()
        if (size > 0 && size + bytes.size > maxBytes) {
            rotate()
            return append(line)
        }
        stream.write(bytes)
        size += bytes.size
    }

    private fun open(): FileOutputStream {
        dir.mkdirs()
        size = current.length()
        return FileOutputStream(current, true).also { out = it }
    }

    private fun rotate() {
        out?.close()
        out = null
        File(dir, "$NAME.$keep").delete()
        for (i in keep - 1 downTo 1) File(dir, "$NAME.$i").renameTo(File(dir, "$NAME.${i + 1}"))
        current.renameTo(File(dir, "$NAME.1"))
    }

    /** Every file's lines in the order they were written. */
    fun readAll(): String = buildString {
        for (i in keep downTo 1) File(dir, "$NAME.$i").takeIf { it.isFile }?.let { append(it.readText()) }
        if (current.isFile) append(current.readText())
    }

    private companion object {
        const val NAME = "app.log"
    }
}

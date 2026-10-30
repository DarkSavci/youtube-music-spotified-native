package dev.darksavci.spotified.data

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import dev.darksavci.spotified.BuildConfig
import java.io.File

/**
 * What crashed, kept for the listener to send if they choose. Nothing leaves
 * the phone on its own: the next launch offers to open a GitHub issue with the
 * report filled in, and the listener reads it before posting.
 */
object Crashes {
    private const val ISSUES = "https://github.com/DarkSavci/youtube-music-spotified-native/issues/new"

    /** The longest stack sent in an issue's address; GitHub turns away longer ones. */
    private const val MAX_STACK = 5_000

    private fun file(context: Context) = File(context.filesDir, "last-crash.txt")

    /** Writes any uncaught exception down before the app dies as it would have. */
    fun install(context: Context) {
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        val app = context.applicationContext
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            runCatching { file(app).writeText(report(thread, error)) }
            previous?.uncaughtException(thread, error)
        }
    }

    /** The report from the last crash, if one was kept. */
    fun pending(context: Context): String? = file(context).takeIf { it.isFile }?.readText()?.takeIf { it.isNotBlank() }

    fun dismiss(context: Context) {
        file(context).delete()
    }

    /** Opens a new GitHub issue with the report in it, for the listener to read and post. */
    fun send(context: Context, report: String) {
        val title = Redact.text(report).lineSequence().firstOrNull { it.startsWith("Exception:") }
            ?.removePrefix("Exception:")?.trim()?.take(80) ?: "Crash"
        // An exception's message can quote an address or a header; it is scrubbed before it can be posted.
        val body = "**What I was doing:**\n\n\n**Report:**\n```\n${Redact.text(report).take(MAX_STACK)}\n```\n"
        val url = Uri.parse(ISSUES).buildUpon()
            .appendQueryParameter("title", "Android crash: $title")
            .appendQueryParameter("body", body)
            .appendQueryParameter("labels", "android,crash")
            .build()
        context.startActivity(Intent(Intent.ACTION_VIEW, url).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        dismiss(context)
    }

    /** The version, the phone and the stack: nothing about the listener or what they play. */
    private fun report(thread: Thread, error: Throwable): String = buildString {
        appendLine("App: ${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})")
        appendLine("Android: ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})")
        appendLine("Phone: ${Build.MANUFACTURER} ${Build.MODEL}")
        appendLine("Thread: ${thread.name}")
        appendLine("Exception: ${error.javaClass.name}: ${error.message.orEmpty().take(200)}")
        appendLine()
        append(error.stackTraceToString())
    }
}

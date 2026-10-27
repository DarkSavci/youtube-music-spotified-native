package dev.darksavci.spotified.spike

import android.annotation.SuppressLint
import android.content.Context
import android.webkit.CookieManager
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import org.json.JSONObject
import java.io.File

private const val START =
    "https://accounts.google.com/ServiceLogin?continue=https%3A%2F%2Fmusic.youtube.com%2F"
private const val MUSIC = "https://music.youtube.com"

/** The cookies the desktop keeps (crates/app/src/signin.rs), and only those. */
private val KEEP = setOf(
    "SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID", "SID", "__Secure-1PSID",
    "__Secure-3PSID", "HSID", "SSID", "APISID", "LOGIN_INFO", "__Secure-1PSIDTS",
    "__Secure-3PSIDTS", "SIDCC", "__Secure-1PSIDCC", "__Secure-3PSIDCC",
    "VISITOR_INFO1_LIVE", "VISITOR_PRIVACY_METADATA", "PREF", "YSC", "__Secure-YNID",
    "__Secure-ROLLOUT_TOKEN", "__Secure-BUCKET", "wide",
)

/**
 * Google's sign-in in a WebView. When it lands on YouTube Music signed in,
 * the cookies become credentials.json, as the desktop's sign-in writes it.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun SignIn(modifier: Modifier = Modifier, onSignedIn: (cookies: Int) -> Unit) {
    AndroidView(modifier = modifier, factory = { context ->
        WebView(context).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            // Google refuses sign-in to a user agent that says it is a
            // WebView; without the marker it reads as Chrome.
            settings.userAgentString = settings.userAgentString
                .replace("; wv", "")
                .replace(Regex("Version/[\\d.]+ "), "")
            webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView, url: String) {
                    if (!url.startsWith(MUSIC)) return
                    val kept = writeCredentials(view.context) ?: return
                    onSignedIn(kept)
                }
            }
            loadUrl(START)
        }
    })
}

/** Writes credentials.json when signed in; the number of cookies kept, or null. */
private fun writeCredentials(context: Context): Int? {
    val header = CookieManager.getInstance().getCookie(MUSIC) ?: return null
    val pairs = header.split("; ")
        .map { it.substringBefore('=') to it }
        .filter { (name, _) -> name in KEEP }
        .distinctBy { it.first }
    val names = pairs.map { it.first }.toSet()
    // An account, and a cookie request signatures are computed over.
    if ("LOGIN_INFO" !in names || names.none { it.endsWith("SAPISID") }) return null
    val json = JSONObject().put("cookie", pairs.joinToString("; ") { it.second })
    val file = File(context.filesDir, "credentials.json")
    val temp = File(context.filesDir, "credentials.json.part")
    temp.writeText(json.toString())
    temp.renameTo(file)
    return pairs.size
}

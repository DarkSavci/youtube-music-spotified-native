package dev.darksavci.spotified.ui.screens

import android.annotation.SuppressLint
import android.graphics.Bitmap
import android.net.Uri
import android.webkit.CookieManager
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.data.Credentials
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.Change
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Starts the core again on what the account folder now holds, and tells every
 * page. The music stops first: what plays belongs to the account being left, and
 * the new one comes back with its own queue, paused.
 */
internal suspend fun Actions.startOnAccount() {
    graph.playback.pause()
    // Answered by the core being left: late, it would reach the new one.
    graph.playback.settled()
    graph.core.restart()
    graph.liked.refresh()
    changed(Change.Everything)
}

/** The accounts on this phone: switch between them, add one, remove one. */
@Composable
internal fun AccountList(signedIn: Boolean) {
    val actions = LocalActions.current
    val p = LocalPalette.current
    val accounts = actions.graph.core.accounts
    var list by remember { mutableStateOf(accounts.list()) }
    var busy by remember { mutableStateOf<String?>(null) }
    val current = accounts.current
    busy?.let { BusyDialog(it) }
    if (list.size > 1) {
        list.forEach { a ->
            val mine = a.id == current
            val hasSignIn = accounts.signedIn(a.id)
            Row(
                Modifier.fillMaxWidth().pressRow(label = "Switch to ${a.name}", enabled = !mine) {
                    val previous = accounts.current
                    busy = "Switching to ${a.name}. The music stops while the app changes accounts."
                    accounts.current = a.id
                    actions.launch {
                        try {
                            actions.startOnAccount()
                            actions.say("Switched to ${a.name}")
                        } catch (e: Exception) {
                            // Back to the account that was working, rather than none.
                            accounts.current = previous
                            runCatching { actions.startOnAccount() }
                            actions.say("Couldn't switch accounts, so you are still on the one you were.")
                        } finally {
                            list = accounts.list()
                            busy = null
                        }
                    }
                }.padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(a.name, style = Type.body.copy(fontSize = 16.sp))
                    if (!hasSignIn) Text("Not signed in", style = Type.small, color = p.secondary)
                }
                if (mine) Glyph(R.drawable.ic_check, "In use", tint = p.accent, size = 20.dp)
                if (a.id.isNotEmpty() && !mine) {
                    GlyphButton(R.drawable.ic_trash_2, "Remove ${a.name}", {
                        actions.confirm(
                            "Remove ${a.name}?",
                            "Its sign-in and its listening history on this phone are deleted. Your YouTube account itself is not touched.",
                            "Remove",
                        ) {
                            accounts.remove(a.id)
                            list = accounts.list()
                            actions.say("Removed ${a.name}")
                        }
                    })
                }
            }
        }
    }
    Action("Add another account", null, R.drawable.ic_plus) { actions.go(Routes.SIGN_IN_NEW) }
    if (signedIn) ChannelList { busy = it }
}

/** The channels of the account in use, when it has more than one to act as. */
@Composable
private fun ChannelList(onBusy: (String?) -> Unit) {
    val actions = LocalActions.current
    val graph = actions.graph
    val p = LocalPalette.current
    val loader = rememberLoader("channels", follows = setOf(Change.Everything)) { graph.api.channels() }
    val state by loader.state.collectAsStateWithLifecycle()
    when (val s = state) {
        is Load.Failed -> if (s.message != ApiError.SignedOut.message) {
            Action("Couldn't read your channels", "Tap to try again", R.drawable.ic_refresh_cw, chevron = false) { loader.reload() }
        }
        is Load.Ready -> {
            val channels = s.value
            if (channels.size > 1) {
                val dir = graph.core.accounts.currentDir()
                val active = remember(channels) { Credentials.read(dir)?.let(Credentials::activeChannel).orEmpty() }
                Text("Your channels", style = Type.small, color = p.secondary, modifier = Modifier.padding(start = 16.dp, top = 12.dp, bottom = 4.dp))
                channels.forEach { c ->
                    val mine = c.id == active
                    Row(
                        Modifier.fillMaxWidth().pressRow(label = "Use ${c.name}", enabled = !mine) {
                            onBusy("Switching to ${c.name}. The music stops while the app changes channels.")
                            actions.launch {
                                try {
                                    withContext(Dispatchers.IO) {
                                        val text = Credentials.read(dir) ?: error("No sign-in")
                                        Credentials.write(dir, Credentials.withChannel(text, c.id))
                                    }
                                    actions.startOnAccount()
                                    actions.say("Now using ${c.name}")
                                } catch (e: Exception) {
                                    actions.say("Couldn't change channels.")
                                } finally {
                                    onBusy(null)
                                }
                            }
                        }.padding(horizontal = 16.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Art(if (c.avatarUrl.isEmpty()) emptyList() else listOf(Artwork(c.avatarUrl, 88, 88)), 32.dp, shape = CircleShape)
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(c.name, style = Type.body)
                            if (c.handle.isNotEmpty()) Text(c.handle, style = Type.small, color = p.secondary)
                        }
                        if (mine) Glyph(R.drawable.ic_check, "In use", tint = p.accent, size = 20.dp)
                    }
                }
            }
            if (channels.isNotEmpty()) {
                Action("Refresh channels", "Look for ones you have just made", R.drawable.ic_refresh_cw, chevron = false) { loader.reload(quietly = true) }
            }
        }
        Load.Pending -> Unit
    }
}

// ---------- signing in ----------

private const val START = "https://accounts.google.com/ServiceLogin?continue=https%3A%2F%2Fmusic.youtube.com%2F"
private const val MUSIC = "https://music.youtube.com"

/**
 * Google's sign-in in a WebView. When it lands on YouTube Music signed in, the
 * cookies become credentials.json, as the desktop's sign-in writes it, and the
 * core starts again with them. With [new], it is for another account: nothing
 * changes until that has signed in, so backing out leaves everything as it was.
 * Google sometimes refuses a WebView, so the desktop's file can stand in.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun SignInScreen(new: Boolean = false) {
    val actions = LocalActions.current
    val graph = actions.graph
    val context = LocalContext.current
    val p = LocalPalette.current
    val scope = rememberCoroutineScope()
    var done by remember { mutableStateOf(false) }
    var working by remember { mutableStateOf(false) }
    var progress by remember { mutableIntStateOf(0) }
    var problem by remember { mutableStateOf<String?>(null) }
    var web by remember { mutableStateOf<WebView?>(null) }

    // The sign-in is written only here, and the account folder made only now that there is one to put in it.
    val finish: (String) -> Unit = { credentials ->
        done = true
        working = true
        problem = null
        scope.launch {
            val accounts = graph.core.accounts
            val previous = accounts.current
            val made = if (new) accounts.add() else null
            try {
                withContext(Dispatchers.IO) { Credentials.write(accounts.currentDir(), credentials) }
                actions.startOnAccount()
                val name = runCatching { graph.api.account()?.name }.getOrNull().orEmpty()
                if (name.isNotEmpty()) accounts.name(accounts.current, name)
                actions.say(if (name.isEmpty()) "Signed in" else "Signed in as $name")
                actions.nav.popBackStack()
            } catch (e: Exception) {
                if (made != null) {
                    accounts.current = previous
                    accounts.remove(made)
                    runCatching { actions.startOnAccount() }
                }
                done = false
                problem = "Signing in didn't finish. Try again."
            } finally {
                working = false
            }
        }
    }

    val file = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            val bytes = withContext(Dispatchers.IO) { runCatching { readLimited(context, uri, Credentials.MOST_BYTES) }.getOrNull() }
            when (val read = if (bytes == null) Credentials.Read.Bad("That file could not be read.") else Credentials.fromFile(String(bytes))) {
                is Credentials.Read.Bad -> actions.say(read.message)
                is Credentials.Read.Good -> finish(read.json)
            }
        }
    }

    if (working) BusyDialog("Signing you in…")
    Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
            Text(if (new) "Add an account" else "Sign in to YouTube Music", style = Type.heading)
        }
        if (progress in 1..99) {
            LinearProgressIndicator({ progress / 100f }, Modifier.fillMaxWidth(), color = p.accent, trackColor = p.surface)
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            AndroidView(
                modifier = Modifier.fillMaxSize(),
                onRelease = { it.destroy() },
                factory = { c ->
                    WebView(c).apply {
                        web = this
                        settings.javaScriptEnabled = true
                        settings.domStorageEnabled = true
                        // Google refuses sign-in to a user agent that says it is a
                        // WebView; without the marker it reads as Chrome.
                        settings.userAgentString = settings.userAgentString.replace("; wv", "").replace(Regex("Version/[\\d.]+ "), "")
                        webChromeClient = object : WebChromeClient() {
                            override fun onProgressChanged(view: WebView, newProgress: Int) {
                                progress = newProgress
                            }
                        }
                        webViewClient = object : WebViewClient() {
                            override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
                                problem = null
                            }

                            override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
                                if (request.isForMainFrame) problem = "Couldn't reach Google. Check your connection and try again."
                            }

                            override fun onPageFinished(view: WebView, url: String) {
                                if (done || !url.startsWith(MUSIC)) return
                                val header = CookieManager.getInstance().getCookie(MUSIC) ?: return
                                val credentials = Credentials.fromCookies(header) ?: return
                                // The file holds the sign-in from here on, so the page's copy is not kept.
                                CookieManager.getInstance().removeAllCookies(null)
                                finish(credentials)
                            }
                        }
                        // Another account starts clean, or Google would simply hand back the one signed in.
                        if (new) CookieManager.getInstance().removeAllCookies { loadUrl(START) } else loadUrl(START)
                    }
                },
            )
            problem?.let { message ->
                Box(Modifier.fillMaxSize().background(p.window), contentAlignment = Alignment.Center) {
                    Problem(message, { problem = null; web?.loadUrl(START) })
                }
            }
        }
        Column(Modifier.fillMaxWidth().padding(16.dp)) {
            Text(
                "Google refusing to sign you in here? Copy credentials.json from the desktop app's data folder to this phone. It is the key to your account: delete the copy afterwards, and never send it to anyone.",
                style = Type.small, color = p.secondary,
            )
            Spacer(Modifier.height(8.dp))
            Pill("Use a file from the desktop", { file.launch(arrayOf("application/json", "text/plain", "*/*")) }, icon = R.drawable.ic_folder)
        }
    }
}

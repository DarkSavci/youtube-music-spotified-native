package dev.darksavci.spotified.ui.screens

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.media.audiofx.Equalizer
import android.net.Uri
import android.os.PowerManager
import android.provider.Settings as SystemSettings
import android.webkit.CookieManager
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.BuildConfig
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Account
import dev.darksavci.spotified.api.CacheUsage
import dev.darksavci.spotified.data.LastFm
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.data.Theme
import dev.darksavci.spotified.data.Updater
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.io.File

@Composable
fun SettingsScreen() {
    val actions = LocalActions.current
    val graph = actions.graph
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val s by graph.settings.settings.collectAsStateWithLifecycle(Settings())
    val account by produceState<Account?>(null) { value = runCatching { graph.api.account() }.getOrNull() }
    // The account list names each account as it was last seen signed in.
    LaunchedEffect(account) {
        account?.name?.takeIf { it.isNotEmpty() }?.let { graph.core.accounts.name(graph.core.accounts.current, it) }
    }
    var cache by remember { mutableStateOf<CacheUsage?>(null) }
    val set: ((Settings) -> Settings) -> Unit = { change -> scope.launch { graph.settings.update(change) } }
    LaunchedEffect(Unit) { cache = runCatching { graph.api.cacheUsage() }.getOrNull() }

    LazyColumn(Modifier.windowInsetsPadding(WindowInsets.statusBars), contentPadding = PaddingValues(bottom = 32.dp)) {
        item {
            Row(verticalAlignment = Alignment.CenterVertically) {
                GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
                Text("Settings", style = Type.title)
            }
        }
        item { Heading("Account") }
        item {
            if (account != null) {
                Line(account!!.name.ifEmpty { "Signed in" }, account!!.handle)
                Action("Sign out") {
                    actions.launch {
                        graph.api.signOut()
                        File(graph.core.accounts.currentDir(), "credentials.json").delete()
                        CookieManager.getInstance().removeAllCookies(null)
                        graph.core.restart()
                        actions.say("Signed out")
                    }
                }
            } else {
                Line("Not signed in", "Sign in for your library, likes and Premium audio")
                Action("Sign in") { actions.go(Routes.SIGN_IN) }
            }
        }

        item { AccountList() }

        item { Heading("Appearance") }
        item {
            LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(Theme.entries) { t -> Chip(t.name, s.theme == t, { set { it.copy(theme = t) } }) }
            }
        }

        item { Heading("Playback") }
        item { Toggle("Gapless", "Run each song into the next", s.gapless) { v -> set { it.copy(gapless = v) } } }
        item {
            SliderRow("Crossfade", if (s.crossfadeMs == 0L) "Off" else "${s.crossfadeMs / 1000} s", s.crossfadeMs / 1000f, 0f..12f) { v ->
                set { it.copy(crossfadeMs = (v.toInt() * 1000).toLong()) }
            }
        }
        item { Toggle("Even out loudness", "Turn loud songs down to the rest", s.normalise) { v -> set { it.copy(normalise = v) } } }
        item { Toggle("Skip silence", "Leave out quiet stretches within a song or episode", s.skipSilence) { v -> set { it.copy(skipSilence = v) } } }
        item { Toggle("SponsorBlock", "Skip the talk and intros listeners have marked in songs from YouTube uploads", s.sponsorBlock) { v -> set { it.copy(sponsorBlock = v) } } }
        item { Toggle("Autoplay", "Carry on with similar songs when the queue ends", s.autoplay) { v -> set { it.copy(autoplay = v) } } }
        item { Toggle("Pick up where you left off", "Keep the queue for the next launch", s.resumeOnLaunch) { v -> set { it.copy(resumeOnLaunch = v) } } }
        item { Toggle("Count plays on YouTube", "Add what you play to your YouTube history", s.reportToYouTube) { v -> set { it.copy(reportToYouTube = v) } } }
        item { Action("Equalizer") { actions.go(Routes.EQUALIZER) } }

        item { Heading("Audio quality") }
        item { QualityRow("On Wi-Fi", s.wifiQuality) { q -> set { it.copy(wifiQuality = q) } } }
        item { QualityRow("On mobile data", s.mobileQuality) { q -> set { it.copy(mobileQuality = q) } } }

        item { Heading("Downloads and storage") }
        item { Toggle("Download on Wi-Fi only", "Kept playlists sync on Wi-Fi", s.downloadOnWifiOnly) { v -> set { it.copy(downloadOnWifiOnly = v) } } }
        item {
            val c = cache
            Line("Song cache", if (c == null) "…" else "${c.tracks} songs, ${c.bytes / 1_048_576} MB")
            SliderRow("Cache size", "${s.cacheMaxMb / 1024.0} GB".replace(".0 ", " "), s.cacheMaxMb / 512f, 1f..16f) { v ->
                set { it.copy(cacheMaxMb = (v.toInt() * 512).toLong()) }
            }
            Action("Empty the cache") { actions.launch { cache = graph.api.clearCache(); actions.say("Emptied; downloads were kept") } }
        }

        item { Heading("Lyrics") }
        item { Toggle("Romanise", "Show lyrics in other scripts in Latin letters", s.lyricsRomanise) { v -> set { it.copy(lyricsRomanise = v) } } }
        item { Toggle("Translate", "Show a translation into your language (sent to Google Translate)", s.lyricsTranslate) { v -> set { it.copy(lyricsTranslate = v) } } }

        item { Heading("Now Playing") }
        item {
            LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(listOf("Spotify", "Full art", "Lyrics first").withIndex().toList()) { (i, name) ->
                    Chip(name, s.nowPlayingStyle == i, { set { it.copy(nowPlayingStyle = i) } })
                }
            }
        }

        item { Heading("Your listening") }
        item { Action("Listening stats") { actions.go(Routes.STATS) } }
        item { Action("Your year") { actions.go(Routes.RECAP) } }
        item { Toggle("New releases", "Tell me when an artist I follow releases something", s.newReleaseAlerts) { v -> set { it.copy(newReleaseAlerts = v) } } }
        item { LastFmSettings(s) }
        item { Action("Import playlists from Spotify, Apple Music or Deezer") { actions.go(Routes.IMPORT) } }
        item { Action("Listen Together") { actions.go(Routes.TOGETHER) } }

        item { Heading("Battery") }
        item {
            val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
            val exempt = pm.isIgnoringBatteryOptimizations(context.packageName)
            Line("Battery optimisation", if (exempt) "Off for this app: playback is not stopped in the background" else "On: some phones stop music in the background")
            if (!exempt) {
                Action("Let it play in the background") {
                    @SuppressLint("BatteryLife")
                    val intent = Intent(SystemSettings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${context.packageName}"))
                    runCatching { context.startActivity(intent) }
                }
            }
            Action("Why music stops on some phones") {
                context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("https://dontkillmyapp.com")))
            }
        }

        item { Heading("About") }
        item { Line("Youtube Music Spotified", "Version ${BuildConfig.VERSION_NAME}. Not affiliated with Spotify or YouTube.") }
        item { Action("Check for updates") { actions.launch { Updater.check(context, graph.http, actions::say) } } }
    }
}

/** The accounts on this phone: switch between them, add one, remove one. */
@Composable
private fun AccountList() {
    val actions = LocalActions.current
    val accounts = actions.graph.core.accounts
    var list by remember { mutableStateOf(accounts.list()) }
    val current = accounts.current
    if (list.size > 1) {
        list.forEach { a ->
            Row(
                Modifier.fillMaxWidth().clickable(enabled = a.id != current) {
                    accounts.current = a.id
                    actions.launch {
                        actions.graph.core.restart()
                        actions.graph.liked.refresh()
                        actions.say("Switched to ${a.name}")
                    }
                }.padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(a.name + if (a.id == current) "  ✓" else "", style = Type.body, modifier = Modifier.weight(1f))
                if (a.id.isNotEmpty() && a.id != current) {
                    GlyphButton(R.drawable.ic_trash_2, "Remove ${a.name}", { accounts.remove(a.id); list = accounts.list() })
                }
            }
        }
    }
    Action("Add another account") {
        accounts.add()
        list = accounts.list()
        actions.launch { actions.graph.core.restart() }
        actions.go(Routes.SIGN_IN)
    }
}

/** Connecting Last.fm: the listener's own key and secret, then Last.fm's page to allow the app. */
@Composable
private fun LastFmSettings(s: Settings) {
    val actions = LocalActions.current
    val graph = actions.graph
    val context = LocalContext.current
    var key by remember(s.lastfmKey) { mutableStateOf(s.lastfmKey) }
    var secret by remember(s.lastfmSecret) { mutableStateOf(s.lastfmSecret) }
    when {
        s.lastfmUser.isNotEmpty() -> {
            Line("Last.fm", "Scrobbling as ${s.lastfmUser}")
            Action("Disconnect Last.fm") { actions.launch { graph.settings.update { it.copy(lastfmSession = "", lastfmUser = "") } } }
        }
        s.lastfmToken.isNotEmpty() -> {
            Line("Last.fm", "Allow the app on Last.fm's page, then come back here")
            Action("I've allowed it") { actions.launch { actions.say("Connected as ${graph.lastFm.finish()}") } }
        }
        else -> {
            Line("Last.fm", "Scrobble what you play. Last.fm gives each app a key of its own: make one, then enter it here.")
            Action("Make a key on Last.fm") { LastFm.authorise(context) }
            OutlinedTextField(key, { key = it }, Modifier.fillMaxWidth().padding(horizontal = 16.dp), label = { Text("API key") }, singleLine = true)
            OutlinedTextField(secret, { secret = it }, Modifier.fillMaxWidth().padding(horizontal = 16.dp), label = { Text("Shared secret") }, singleLine = true)
            Action("Connect") { actions.launch { graph.lastFm.start(key, secret) } }
        }
    }
}

/** Highest is Premium's 256 kbps where there is one; standard about 160; data saver about 70. */
@Composable
private fun QualityRow(title: String, current: String, onPick: (String) -> Unit) {
    Column(Modifier.padding(vertical = 4.dp)) {
        Text(title, style = Type.body, modifier = Modifier.padding(horizontal = 16.dp))
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            items(listOf("high" to "Highest", "standard" to "Standard", "low" to "Data saver")) { (q, label) ->
                Chip(label, current == q, { onPick(q) })
            }
        }
    }
}

@Composable
private fun Heading(text: String) {
    Text(text, style = Type.heading, modifier = Modifier.padding(start = 16.dp, top = 24.dp, bottom = 8.dp))
}

@Composable
private fun Line(title: String, subtitle: String) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Text(title, style = Type.body)
        if (subtitle.isNotEmpty()) Text(subtitle, style = Type.small, color = LocalPalette.current.secondary)
    }
}

@Composable
private fun Action(label: String, onClick: () -> Unit) {
    Text(
        label, style = Type.bodyStrong, color = LocalPalette.current.accent,
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 12.dp),
    )
}

@Composable
private fun Toggle(title: String, subtitle: String, on: Boolean, onChange: (Boolean) -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().clickable { onChange(!on) }.padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = Type.body)
            Text(subtitle, style = Type.small, color = p.secondary)
        }
        Switch(on, onChange, colors = SwitchDefaults.colors(checkedTrackColor = p.accent))
    }
}

@Composable
private fun SliderRow(title: String, value: String, current: Float, range: ClosedFloatingPointRange<Float>, onDone: (Float) -> Unit) {
    val p = LocalPalette.current
    var v by remember(current) { mutableFloatStateOf(current) }
    Column(Modifier.padding(horizontal = 16.dp, vertical = 6.dp)) {
        Row {
            Text(title, style = Type.body, modifier = Modifier.weight(1f))
            Text(value, style = Type.small, color = p.secondary)
        }
        Slider(
            v, { v = it }, valueRange = range, onValueChangeFinished = { onDone(v) },
            steps = (range.endInclusive - range.start).toInt() - 1,
            colors = SliderDefaults.colors(thumbColor = p.accent, activeTrackColor = p.accent),
        )
    }
}

// ---------- signing in ----------

private const val START = "https://accounts.google.com/ServiceLogin?continue=https%3A%2F%2Fmusic.youtube.com%2F"
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
 * the cookies become credentials.json, as the desktop's sign-in writes it,
 * and the core starts again with them.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun SignInScreen() {
    val actions = LocalActions.current
    var done by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
            Text("Sign in to YouTube Music", style = Type.heading)
        }
        AndroidView(modifier = Modifier.fillMaxSize(), factory = { context ->
            WebView(context).apply {
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                // Google refuses sign-in to a user agent that says it is a
                // WebView; without the marker it reads as Chrome.
                settings.userAgentString = settings.userAgentString.replace("; wv", "").replace(Regex("Version/[\\d.]+ "), "")
                webViewClient = object : WebViewClient() {
                    override fun onPageFinished(view: WebView, url: String) {
                        if (done || !url.startsWith(MUSIC)) return
                        val kept = writeCredentials(view.context) ?: return
                        done = true
                        actions.launch {
                            actions.graph.core.restart()
                            actions.graph.liked.refresh()
                            actions.say("Signed in ($kept cookies kept)")
                        }
                        actions.nav.popBackStack()
                    }
                }
                loadUrl(START)
            }
        })
    }
}

/** Writes credentials.json when signed in; the number of cookies kept, or null. */
private fun writeCredentials(context: Context): Int? {
    val header = CookieManager.getInstance().getCookie(MUSIC) ?: return null
    val pairs = header.split("; ").map { it.substringBefore('=') to it }.filter { (name, _) -> name in KEEP }.distinctBy { it.first }
    val names = pairs.map { it.first }.toSet()
    // An account, and a cookie request signatures are computed over.
    if ("LOGIN_INFO" !in names || names.none { it.endsWith("SAPISID") }) return null
    val json = JSONObject().put("cookie", pairs.joinToString("; ") { it.second })
    val dir = context.graph.core.accounts.currentDir()
    val file = File(dir, "credentials.json")
    val temp = File(dir, "credentials.json.part")
    temp.writeText(json.toString())
    temp.renameTo(file)
    return pairs.size
}

// ---------- the equalizer ----------

private val PRESETS = mapOf(
    "Flat" to listOf(0, 0, 0, 0, 0),
    "Bass" to listOf(600, 300, 0, 0, 0),
    "Treble" to listOf(0, 0, 0, 300, 600),
    "Vocal" to listOf(-200, 0, 400, 200, -100),
    "Rock" to listOf(400, 200, -100, 200, 400),
)

/** The phone's own equalizer, on the engine's audio session: its bands, as many as it has. */
@Composable
fun EqualizerScreen() {
    val actions = LocalActions.current
    val graph = actions.graph
    val scope = rememberCoroutineScope()
    val p = LocalPalette.current
    val s by graph.settings.settings.collectAsStateWithLifecycle(Settings())
    val bands = remember {
        runCatching {
            val eq = Equalizer(-1, graph.playback.engine?.audioSessionId ?: 0)
            val info = (0 until eq.numberOfBands).map { b -> eq.getCenterFreq(b.toShort()) / 1000 }
            val range = eq.bandLevelRange.let { it[0].toInt() to it[1].toInt() }
            eq.release()
            info to range
        }.getOrNull()
    }
    val levels = s.equalizerBands.split(',').mapNotNull { it.trim().toIntOrNull() }
    val save: (List<Int>) -> Unit = { l -> scope.launch { graph.settings.update { it.copy(equalizerBands = l.joinToString(",")) } } }
    LazyColumn(Modifier.windowInsetsPadding(WindowInsets.statusBars)) {
        item {
            Row(verticalAlignment = Alignment.CenterVertically) {
                GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
                Text("Equalizer", style = Type.title, modifier = Modifier.weight(1f))
                Switch(s.equalizerOn, { on -> scope.launch { graph.settings.update { it.copy(equalizerOn = on) } } }, colors = SwitchDefaults.colors(checkedTrackColor = p.accent))
                Spacer(Modifier.width(16.dp))
            }
        }
        if (bands == null) {
            item { Line("Not available", "This phone does not offer an equalizer to apps.") }
            return@LazyColumn
        }
        val (freqs, range) = bands
        item {
            LazyRow(contentPadding = PaddingValues(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(PRESETS.keys.toList()) { name ->
                    Pill(name, { save(PRESETS.getValue(name).let { pre -> freqs.indices.map { pre.getOrElse(it * pre.size / freqs.size) { 0 } } }) })
                }
            }
        }
        items(freqs.indices.toList()) { b ->
            val level = levels.getOrElse(b) { 0 }
            var v by remember(level) { mutableFloatStateOf(level.toFloat()) }
            Column(Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
                Row {
                    Text(if (freqs[b] >= 1000) "${freqs[b] / 1000} kHz" else "${freqs[b]} Hz", style = Type.body, modifier = Modifier.weight(1f))
                    Text("%+.1f dB".format(v / 100f), style = Type.small, color = p.secondary)
                }
                Slider(
                    v, { v = it }, valueRange = range.first.toFloat()..range.second.toFloat(),
                    onValueChangeFinished = { save(freqs.indices.map { i -> if (i == b) v.toInt() else levels.getOrElse(i) { 0 } }) },
                    enabled = s.equalizerOn,
                    colors = SliderDefaults.colors(thumbColor = p.accent, activeTrackColor = p.accent),
                )
            }
        }
        item {
            Action("Open the system's equalizer") {
                val intent = Intent("android.media.action.DISPLAY_AUDIO_EFFECT_CONTROL_PANEL")
                    .putExtra("android.media.extra.AUDIO_SESSION", graph.playback.engine?.audioSessionId ?: 0)
                    .putExtra("android.media.extra.CONTENT_TYPE", 0)
                runCatching { actions.nav.context.startActivity(intent) }.onFailure { actions.say("This phone has no equalizer of its own.") }
            }
        }
    }
}

package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.ui.components.ThinSlider
import android.annotation.SuppressLint
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import kotlin.math.abs
import androidx.annotation.DrawableRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.darksavci.spotified.ui.components.Glyph
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

    val signOut: () -> Unit = {
        actions.launch {
            graph.api.signOut()
            File(graph.core.accounts.currentDir(), "credentials.json").delete()
            CookieManager.getInstance().removeAllCookies(null)
            graph.core.restart()
            actions.say("Signed out")
        }
    }

    LazyColumn(Modifier.windowInsetsPadding(WindowInsets.statusBars), contentPadding = PaddingValues(bottom = 40.dp)) {
        item {
            Row(Modifier.padding(end = 16.dp, top = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
                Text("Settings", style = Type.bodyStrong.copy(fontSize = 17.sp))
            }
        }
        item { ProfileCard(account) { actions.go(Routes.SIGN_IN) } }
        item { AccountList() }

        item { Heading("Appearance") }
        item {
            Choice("Theme", Theme.entries.map { it to it.name }, s.theme) { t -> set { it.copy(theme = t) } }
        }
        item {
            Choice("Now Playing", listOf(0 to "Spotify", 1 to "Full art", 2 to "Lyrics first"), s.nowPlayingStyle) { i -> set { it.copy(nowPlayingStyle = i) } }
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
        item { Toggle("SponsorBlock", "Skip the talk and intros listeners have marked in YouTube uploads", s.sponsorBlock) { v -> set { it.copy(sponsorBlock = v) } } }
        item { Toggle("Autoplay", "Carry on with similar songs when the queue ends", s.autoplay) { v -> set { it.copy(autoplay = v) } } }
        item { Toggle("Pick up where you left off", "Keep the queue for the next launch", s.resumeOnLaunch) { v -> set { it.copy(resumeOnLaunch = v) } } }
        item { Toggle("Count plays on YouTube", "Add what you play to your YouTube history", s.reportToYouTube) { v -> set { it.copy(reportToYouTube = v) } } }
        item { Action("Equalizer", "Shape the sound by band", R.drawable.ic_sliders_vertical) { actions.go(Routes.EQUALIZER) } }

        item { Heading("Audio quality") }
        item { Choice("On Wi-Fi", QUALITIES, s.wifiQuality) { q -> set { it.copy(wifiQuality = q) } } }
        item { Choice("On mobile data", QUALITIES, s.mobileQuality) { q -> set { it.copy(mobileQuality = q) } } }

        item { Heading("Downloads and storage") }
        item { Toggle("Download on Wi-Fi only", "Kept playlists sync on Wi-Fi", s.downloadOnWifiOnly) { v -> set { it.copy(downloadOnWifiOnly = v) } } }
        item {
            val c = cache
            SliderRow(
                "Song cache", "${s.cacheMaxMb / 1024.0} GB".replace(".0 ", " "), s.cacheMaxMb / 512f, 1f..16f,
                subtitle = if (c == null) "Counting\u2026" else "${c.tracks} songs, ${c.bytes / 1_048_576} MB used",
            ) { v -> set { it.copy(cacheMaxMb = (v.toInt() * 512).toLong()) } }
        }
        item {
            Action("Empty the cache", "Downloads are kept", R.drawable.ic_trash_2, chevron = false) {
                actions.launch { cache = graph.api.clearCache(); actions.say("Emptied; downloads were kept") }
            }
        }

        item { Heading("Lyrics") }
        item { Toggle("Romanise", "Show lyrics in other scripts in Latin letters", s.lyricsRomanise) { v -> set { it.copy(lyricsRomanise = v) } } }
        item { Toggle("Translate", "Show a translation into your language (sent to Google Translate)", s.lyricsTranslate) { v -> set { it.copy(lyricsTranslate = v) } } }

        item { Heading("Your listening") }
        item { Action("Listening stats", "Your top songs, artists and hours", R.drawable.ic_audio_lines) { actions.go(Routes.STATS) } }
        item { Action("Your year", "Your year in music, as a story", R.drawable.ic_gift) { actions.go(Routes.RECAP) } }
        item { Toggle("New releases", "Tell me when an artist I follow releases something", s.newReleaseAlerts) { v -> set { it.copy(newReleaseAlerts = v) } } }
        item { Action("Import playlists", "From Spotify, Apple Music or Deezer", R.drawable.ic_list_plus) { actions.go(Routes.IMPORT) } }
        item { Action("Listen Together", "Play in step with friends", R.drawable.ic_users) { actions.go(Routes.TOGETHER) } }

        item { Heading("Battery") }
        item {
            val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
            val exempt = pm.isIgnoringBatteryOptimizations(context.packageName)
            if (exempt) {
                Line("Background playback", "Battery optimisation is off for this app, so music keeps playing")
            } else {
                Action("Let it play in the background", "Battery optimisation can stop music on some phones", R.drawable.ic_headphones) {
                    @SuppressLint("BatteryLife")
                    val intent = Intent(SystemSettings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${context.packageName}"))
                    runCatching { context.startActivity(intent) }
                }
            }
        }
        item {
            Action("Why music stops on some phones", "dontkillmyapp.com", R.drawable.ic_external_link) {
                context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("https://dontkillmyapp.com")))
            }
        }

        item { Heading("About") }
        item { Line("Version", "${BuildConfig.VERSION_NAME}. Not affiliated with Spotify or YouTube.") }
        item { Action("Check for updates", null, R.drawable.ic_refresh_cw) { actions.launch { Updater.check(context, graph.http, actions::say) } } }

        if (account != null) {
            item {
                Box(Modifier.fillMaxWidth().padding(top = 32.dp), contentAlignment = Alignment.Center) {
                    Text(
                        "Log out", style = Type.bodyStrong, color = LocalPalette.current.window,
                        modifier = Modifier.clip(RoundedCornerShape(24.dp)).background(LocalPalette.current.text)
                            .clickable(onClick = signOut).padding(horizontal = 32.dp, vertical = 12.dp),
                    )
                }
            }
        }
    }
}

private val QUALITIES = listOf("high" to "Highest", "standard" to "Standard", "low" to "Data saver")

/** Who is signed in, picture and name, as Spotify's settings open; or a way to sign in. */
@Composable
private fun ProfileCard(account: Account?, onSignIn: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.padding(horizontal = 16.dp, vertical = 8.dp).fillMaxWidth().clip(RoundedCornerShape(12.dp))
            .background(p.surface).clickable(enabled = account == null, onClick = onSignIn).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(56.dp).clip(CircleShape).background(p.surfaceActive), contentAlignment = Alignment.Center) {
            if (account?.avatarUrl.isNullOrEmpty()) {
                Glyph(R.drawable.ic_user, null, tint = p.secondary, size = 28.dp)
            } else {
                AsyncImage(account!!.avatarUrl, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            }
        }
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(account?.name?.ifEmpty { "Signed in" } ?: "Not signed in", style = Type.bodyStrong.copy(fontSize = 18.sp))
            Text(
                account?.handle?.ifEmpty { "YouTube Music" } ?: "Sign in for your library, likes and Premium audio",
                style = Type.small, color = p.secondary,
            )
        }
        if (account == null) Glyph(R.drawable.ic_chevron_right, null, tint = p.secondary, size = 20.dp)
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
                Text(a.name, style = Type.body.copy(fontSize = 16.sp), modifier = Modifier.weight(1f))
                if (a.id == current) Glyph(R.drawable.ic_check, "Current", tint = LocalPalette.current.accent, size = 20.dp)
                if (a.id.isNotEmpty() && a.id != current) {
                    GlyphButton(R.drawable.ic_trash_2, "Remove ${a.name}", { accounts.remove(a.id); list = accounts.list() })
                }
            }
        }
    }
    Action("Add another account", null, R.drawable.ic_plus) {
        accounts.add()
        list = accounts.list()
        actions.launch { actions.graph.core.restart() }
        actions.go(Routes.SIGN_IN)
    }
}

@Composable
private fun Heading(text: String) {
    Text(text, style = Type.bodyStrong.copy(fontSize = 19.sp), modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 28.dp, bottom = 6.dp))
}

@Composable
private fun Line(title: String, subtitle: String) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
        Text(title, style = Type.body.copy(fontSize = 16.sp))
        if (subtitle.isNotEmpty()) Text(subtitle, style = Type.small, color = LocalPalette.current.secondary)
    }
}

/** A row that does something: an icon, what it does, and a chevron when it opens a page. */
@Composable
private fun Action(title: String, subtitle: String? = null, @DrawableRes icon: Int? = null, chevron: Boolean = true, onClick: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Glyph(icon, null, tint = p.text, size = 22.dp)
            Spacer(Modifier.width(16.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(title, style = Type.body.copy(fontSize = 16.sp))
            if (!subtitle.isNullOrEmpty()) Text(subtitle, style = Type.small, color = p.secondary)
        }
        if (chevron) Glyph(R.drawable.ic_chevron_right, null, tint = p.secondary, size = 20.dp)
    }
}

@Composable
private fun switchColors() = LocalPalette.current.let { p ->
    SwitchDefaults.colors(
        checkedTrackColor = p.accent, checkedThumbColor = Color.White, checkedBorderColor = Color.Transparent,
        uncheckedTrackColor = p.surfaceActive, uncheckedThumbColor = p.secondary, uncheckedBorderColor = Color.Transparent,
    )
}

@Composable
private fun Toggle(title: String, subtitle: String, on: Boolean, onChange: (Boolean) -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().clickable { onChange(!on) }.padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(title, style = Type.body.copy(fontSize = 16.sp))
            Text(subtitle, style = Type.small, color = p.secondary)
        }
        Switch(on, onChange, colors = switchColors())
    }
}

/** One of a few options, as a segmented control under its title. */
@Composable
private fun <T> Choice(title: String, options: List<Pair<T, String>>, current: T, onPick: (T) -> Unit) {
    val p = LocalPalette.current
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Text(title, style = Type.body.copy(fontSize = 16.sp), modifier = Modifier.padding(bottom = 8.dp))
        Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(p.surface).padding(4.dp)) {
            options.forEach { (value, label) ->
                val on = value == current
                Box(
                    Modifier.weight(1f).clip(RoundedCornerShape(8.dp)).background(if (on) p.surfaceActive else Color.Transparent)
                        .clickable { onPick(value) }.padding(vertical = 10.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(label, style = if (on) Type.label.copy(fontWeight = FontWeight.Bold) else Type.label, color = if (on) p.text else p.secondary, maxLines = 1)
                }
            }
        }
    }
}

@Composable
private fun SliderRow(
    title: String, value: String, current: Float, range: ClosedFloatingPointRange<Float>,
    subtitle: String? = null, onDone: (Float) -> Unit,
) {
    val p = LocalPalette.current
    var v by remember(current) { mutableFloatStateOf(current) }
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(title, style = Type.body.copy(fontSize = 16.sp))
                if (!subtitle.isNullOrEmpty()) Text(subtitle, style = Type.small, color = p.secondary)
            }
            Text(value, style = Type.bodyStrong, color = p.text)
        }
        // Whole steps: seconds of crossfade, half-gigabytes of cache.
        ThinSlider(v, { v = Math.round(it).toFloat() }, { onDone(v) }, range)
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

/**
 * The bands as a curve, Spotify's way: a point per band, dragged up or down,
 * with the line through them and the area under it in the accent colour.
 * The level is saved when the finger lifts.
 */
@Composable
private fun EqualizerCurve(freqs: List<Int>, levels: List<Int>, range: Pair<Int, Int>, enabled: Boolean, onSet: (Int, Int) -> Unit) {
    val p = LocalPalette.current
    val live = remember(levels) { levels.toMutableStateList() }
    val tint = if (enabled) p.accent else p.secondary
    val (low, high) = range
    Column(Modifier.padding(horizontal = 16.dp).fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(p.surface).padding(vertical = 16.dp)) {
        BoxWithConstraints(Modifier.fillMaxWidth().height(220.dp)) {
            val w = constraints.maxWidth.toFloat()
            val h = constraints.maxHeight.toFloat()
            val pad = with(LocalDensity.current) { 24.dp.toPx() }
            // Each point over its label: the labels below share the width in equal columns.
            val inner = w - 2 * with(LocalDensity.current) { 8.dp.toPx() }
            val xOf = { i: Int -> (w - inner) / 2 + inner * (i + 0.5f) / freqs.size }
            val yOf = { level: Int -> pad + (h - 2 * pad) * (1f - (level - low).toFloat() / (high - low).coerceAtLeast(1)) }
            val levelOf = { y: Float -> (low + (1f - ((y - pad) / (h - 2 * pad)).coerceIn(0f, 1f)) * (high - low)).toInt() }
            var dragging by remember { mutableIntStateOf(-1) }
            Canvas(
                Modifier.fillMaxSize().pointerInput(enabled, freqs.size) {
                    if (!enabled) return@pointerInput
                    detectDragGestures(
                        onDragStart = { at -> dragging = freqs.indices.minByOrNull { abs(xOf(it) - at.x) } ?: -1 },
                        onDragEnd = { if (dragging >= 0) onSet(dragging, live[dragging]); dragging = -1 },
                        onDragCancel = { dragging = -1 },
                    ) { change, _ ->
                        change.consume()
                        if (dragging >= 0) live[dragging] = levelOf(change.position.y)
                    }
                },
            ) {
                // The 0 dB line, faint.
                val zero = yOf(0)
                drawLine(p.outline, Offset(xOf(0), zero), Offset(xOf(freqs.size - 1), zero), strokeWidth = 2f)
                val points = live.indices.map { Offset(xOf(it), yOf(live[it])) }
                val line = Path().apply {
                    moveTo(points.first().x, points.first().y)
                    for (i in 1 until points.size) {
                        val a = points[i - 1]
                        val b = points[i]
                        val mid = (a.x + b.x) / 2
                        cubicTo(mid, a.y, mid, b.y, b.x, b.y)
                    }
                }
                val area = Path().apply {
                    addPath(line)
                    lineTo(points.last().x, h - pad)
                    lineTo(points.first().x, h - pad)
                    close()
                }
                drawPath(area, Brush.verticalGradient(listOf(tint.copy(alpha = 0.35f), tint.copy(alpha = 0f)), startY = pad, endY = h - pad))
                drawPath(line, tint, style = Stroke(width = 6f, cap = StrokeCap.Round))
                points.forEachIndexed { i, pt ->
                    drawCircle(Color.White, radius = if (i == dragging) 18f else 13f, center = pt)
                    drawCircle(tint, radius = if (i == dragging) 11f else 7f, center = pt)
                }
            }
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp)) {
            freqs.forEachIndexed { i, f ->
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(if (f >= 1000) "${f / 1000} kHz" else "$f Hz", style = Type.label, color = p.text)
                    Text("%+.1f".format(live[i] / 100f), style = Type.tiny, color = p.secondary)
                }
            }
        }
    }
}

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
                Switch(s.equalizerOn, { on -> scope.launch { graph.settings.update { it.copy(equalizerOn = on) } } }, colors = switchColors())
                Spacer(Modifier.width(16.dp))
            }
        }
        if (bands == null) {
            item { Line("Not available", "This phone does not offer an equalizer to apps.") }
            return@LazyColumn
        }
        val (freqs, range) = bands
        // A preset is lit while the bands are what it sets.
        val presetLevels = { pre: List<Int> -> freqs.indices.map { pre.getOrElse(it * pre.size / freqs.size) { 0 } } }
        val current = freqs.indices.map { levels.getOrElse(it) { 0 } }
        item {
            LazyRow(contentPadding = PaddingValues(16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(PRESETS.keys.toList()) { name ->
                    val levelsOf = presetLevels(PRESETS.getValue(name))
                    Chip(name, s.equalizerOn && levelsOf == current, { save(levelsOf) })
                }
            }
        }
        item {
            EqualizerCurve(freqs, current, range, s.equalizerOn) { b, level -> save(current.mapIndexed { i, l -> if (i == b) level else l }) }
        }
        item {
            Text(
                if (s.equalizerOn) "Drag a point up or down to shape that band." else "Switch it on to shape the sound.",
                style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
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

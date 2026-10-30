package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.ui.components.SearchField
import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.media.audiofx.Equalizer
import android.net.Uri
import android.os.PowerManager
import android.provider.Settings as SystemSettings
import android.webkit.CookieManager
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import coil3.compose.AsyncImage
import dev.darksavci.spotified.BuildConfig
import dev.darksavci.spotified.ui.player.openOutputPicker
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Account
import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.car.AutoSetup
import dev.darksavci.spotified.data.Credentials
import dev.darksavci.spotified.data.RemotePickup
import dev.darksavci.spotified.data.Report
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.data.Theme
import dev.darksavci.spotified.data.Updater
import dev.darksavci.spotified.data.resetKeepingYours
import dev.darksavci.spotified.data.sizeText
import dev.darksavci.spotified.ui.Change
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.predictiveBack
import dev.darksavci.spotified.ui.components.predictiveBackLayer
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import java.io.File
import java.util.Locale
import kotlin.math.abs
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** One setting: which section it lives in, what it is called and other words it answers to, and the row itself. */
private class SettingEntry(val section: String, val title: String, val words: String = "", val row: @Composable () -> Unit)

/** The sections, in order, with what each row on the first page says of them. */
private val SECTIONS = listOf(
    Triple("Appearance", "Theme • Now Playing style • Motion", R.drawable.ic_layout_grid),
    Triple("Playback", "Crossfade • Autoplay • Blocked", R.drawable.ic_play),
    Triple("Sound and quality", "Volume • Bass • Equalizer • Audio quality", R.drawable.ic_volume_2),
    Triple("Downloads and storage", "Downloads • Song cache", R.drawable.ic_download),
    Triple("Lyrics", "Romanise • Translate", R.drawable.ic_mic_vocal),
    Triple("Your listening", "Stats • Your year • New releases • Import", R.drawable.ic_gift),
    Triple("Battery", "Background playback", R.drawable.ic_headphones),
    Triple("About", "Version • Updates • Report a problem", R.drawable.ic_info),
)

/** Whether every word typed appears in a setting's names; the section's own name counts. */
internal fun settingMatches(query: String, vararg names: String): Boolean {
    val all = names.joinToString(" ").lowercase()
    val words = query.trim().lowercase().split(Regex("\\s+")).filter { it.isNotEmpty() }
    return words.isNotEmpty() && words.all { it in all }
}

private val QUALITIES = listOf("high" to "Highest", "standard" to "Standard", "low" to "Data saver")

@Composable
fun SettingsScreen() {
    val actions = LocalActions.current
    val graph = actions.graph
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val p = LocalPalette.current
    val s by graph.settings.settings.collectAsStateWithLifecycle(Settings())
    val update by Updater.status.collectAsStateWithLifecycle()
    val accountLoader = rememberLoader("account", follows = setOf(Change.Everything)) { graph.api.account() }
    val accountState by accountLoader.state.collectAsStateWithLifecycle()
    val account = (accountState as? Load.Ready)?.value
    // The account list names each account as it was last seen signed in.
    LaunchedEffect(account) {
        account?.name?.takeIf { it.isNotEmpty() }?.let { graph.core.accounts.name(graph.core.accounts.current, it) }
    }
    val cacheLoader = rememberLoader("cache-usage") { graph.api.cacheUsage() }
    val cacheState by cacheLoader.state.collectAsStateWithLifecycle()
    val downloadBytes by graph.downloads.bytes.collectAsStateWithLifecycle()
    val set: ((Settings) -> Settings) -> Unit = { change -> scope.launch { graph.settings.update(change) } }
    var busy by remember { mutableStateOf<String?>(null) }
    busy?.let { BusyDialog(it) }

    val signOut: () -> Unit = {
        busy = "Signing out…"
        actions.launch {
            try {
                // Whatever the core says, the sign-in on the phone goes: that is what was asked.
                runCatching { graph.api.signOut() }
                File(graph.core.accounts.currentDir(), Credentials.FILE).delete()
                withContext(Dispatchers.Main) { CookieManager.getInstance().removeAllCookies(null) }
                actions.startOnAccount()
                actions.say("Signed out")
            } finally {
                busy = null
            }
        }
    }

    val entries = buildList {
        add(SettingEntry("Appearance", "Theme", "dark black light colour") {
            Choice("Theme", Theme.entries.map { it to it.name }, s.theme) { t -> set { it.copy(theme = t) } }
        })
        add(SettingEntry("Appearance", "Now Playing", "player style full art lyrics first") {
            Choice("Now Playing", listOf(0 to "Spotify", 1 to "Full art", 2 to "Lyrics first"), s.nowPlayingStyle) { i -> set { it.copy(nowPlayingStyle = i) } }
        })
        add(SettingEntry("Appearance", "Reduce motion", "animation transitions") {
            Toggle("Reduce motion", "Change pages without sliding or fading", s.reduceMotion) { v -> set { it.copy(reduceMotion = v) } }
        })
        add(SettingEntry("Appearance", "Visualizer", "bars spectrum animation now playing") {
            Toggle("Visualizer", "Bars that move with the music on Now Playing", s.visualizer) { v -> set { it.copy(visualizer = v) } }
        })

        add(SettingEntry("Playback", "Gapless") { Toggle("Gapless", "Run each song into the next", s.gapless) { v -> set { it.copy(gapless = v) } } })
        add(SettingEntry("Playback", "Crossfade", "fade") {
            SliderRow("Crossfade", if (s.crossfadeMs == 0L) "Off" else "${s.crossfadeMs / 1000} s", s.crossfadeMs / 1000f, 0f..12f) { v ->
                set { it.copy(crossfadeMs = (v.toInt() * 1000).toLong()) }
            }
        })
        add(SettingEntry("Playback", "Autoplay", "radio similar") { Toggle("Autoplay", "Carry on with similar songs when the queue ends", s.autoplay) { v -> set { it.copy(autoplay = v) } } })
        add(SettingEntry("Playback", "Skip silence", "quiet") { Toggle("Skip silence", "Leave out quiet stretches within a song or episode", s.skipSilence) { v -> set { it.copy(skipSilence = v) } } })
        add(SettingEntry("Playback", "SponsorBlock", "sponsor ads intro") { Toggle("SponsorBlock", "Skip the talk and intros listeners have marked in YouTube uploads", s.sponsorBlock) { v -> set { it.copy(sponsorBlock = v) } } })
        add(SettingEntry("Playback", "Pick up where you left off", "resume queue launch") { Toggle("Pick up where you left off", "Keep the queue for the next launch", s.resumeOnLaunch) { v -> set { it.copy(resumeOnLaunch = v) } } })
        add(SettingEntry("Playback", "Continue from YouTube Music", "other device remote queue") {
            Toggle("Continue from YouTube Music", "When the app starts with nothing playing, bring in the queue from your other devices, paused", s.continueFromYouTubeMusic) { v -> set { it.copy(continueFromYouTubeMusic = v) } }
        })
        add(SettingEntry("Playback", "Bring in my queue now", "continue youtube music remote device") {
            Action("Bring in my queue now", "From YouTube Music on your other devices. It replaces this queue", R.drawable.ic_list_end, chevron = false) {
                actions.launch {
                    val found = RemotePickup.pick(graph.api.remoteQueue())
                    if (found == null) actions.say("Nothing is queued on your other devices.")
                    else graph.playback.send(Command.Play(found.tracks, found.index, found.origin))
                }
            }
        })
        add(SettingEntry("Playback", "Android Auto", "car setup unknown sources steps") {
            Action("Android Auto", "The steps that make the app show up in the car", R.drawable.ic_headphones) { AutoSetup.show() }
        })
        add(SettingEntry("Playback", "Count plays on YouTube", "history report") { Toggle("Count plays on YouTube", "Add what you play to your YouTube history", s.reportToYouTube) { v -> set { it.copy(reportToYouTube = v) } } })
        add(SettingEntry("Playback", "Blocked", "never play unblock hide") {
            val n = s.blocked.songs.size + s.blocked.artists.size + s.blocked.albums.size
            Action("Blocked", if (n == 0) "Nothing is blocked" else if (n == 1) "1 thing is never played" else "$n things are never played", R.drawable.ic_ban) { actions.go(Routes.BLOCKED) }
        })

        add(SettingEntry("Sound and quality", "Even out loudness", "normalise normalize volume") { Toggle("Even out loudness", "Turn loud songs down to the rest", s.normalise) { v -> set { it.copy(normalise = v) } } })
        add(SettingEntry("Sound and quality", "Volume level", "loud quiet normal") {
            Choice("Volume level", listOf("quiet" to "Quiet", "normal" to "Normal", "loud" to "Loud"), s.volumeLevel, "How loud songs are left once evened out") { v -> set { it.copy(volumeLevel = v) } }
        })
        add(SettingEntry("Sound and quality", "Volume boost", "louder") { Toggle("Volume boost", "Louder than the phone's volume alone gives. Loud songs may be held back to avoid distortion", s.volumeBoost) { v -> set { it.copy(volumeBoost = v) } } })
        add(SettingEntry("Sound and quality", "Bass", "boost car") { Toggle("Bass", "More low end, for a car or small speakers", s.bassBoost) { v -> set { it.copy(bassBoost = v) } } })
        add(SettingEntry("Sound and quality", "Equalizer", "bands treble eq") { Action("Equalizer", "Shape the sound by band", R.drawable.ic_sliders_vertical) { actions.go(Routes.EQUALIZER) } })
        add(SettingEntry("Sound and quality", "Output device", "bluetooth speaker headphones cast") { Action("Output device", "Choose where the sound goes", R.drawable.ic_headphones) { openOutputPicker(context) } })
        add(SettingEntry("Sound and quality", "On Wi-Fi", "audio quality wifi") { Choice("Quality on Wi-Fi", QUALITIES, s.wifiQuality) { q -> set { it.copy(wifiQuality = q) } } })
        add(SettingEntry("Sound and quality", "On mobile data", "audio quality cellular data saver") { Choice("Quality on mobile data", QUALITIES, s.mobileQuality) { q -> set { it.copy(mobileQuality = q) } } })

        add(SettingEntry("Downloads and storage", "Downloads", "offline songs") { Action("Downloads", "Songs kept on this phone, ${sizeText(downloadBytes)}", R.drawable.ic_download) { actions.go(Routes.DOWNLOADS) } })
        add(SettingEntry("Downloads and storage", "Music on this phone", "local files") { Action("Music on this phone", "Your own files", R.drawable.ic_folder) { actions.go(Routes.LOCAL) } })
        add(SettingEntry("Downloads and storage", "Download on Wi-Fi only", "sync mobile data") { Toggle("Download on Wi-Fi only", "Songs download and kept playlists sync on Wi-Fi", s.downloadOnWifiOnly) { v -> set { it.copy(downloadOnWifiOnly = v) } } })
        add(SettingEntry("Downloads and storage", "Song cache", "storage size space") {
            val gb = String.format(Locale.US, "%.1f GB", s.cacheMaxMb / 1024.0).replace(".0 ", " ")
            val usage = when (val c = cacheState) {
                is Load.Ready -> "${c.value.tracks} songs, ${sizeText(c.value.bytes)} used, ${sizeText(downloadBytes)} of it downloads"
                is Load.Failed -> "Couldn't count what is kept"
                Load.Pending -> "Counting…"
            }
            SliderRow("Song cache", gb, s.cacheMaxMb / 512f, 1f..16f, subtitle = usage) { v -> set { it.copy(cacheMaxMb = (v.toInt() * 512).toLong()) } }
        })
        add(SettingEntry("Downloads and storage", "Empty the cache", "clear storage space delete") {
            Action("Empty the cache", "Downloads are kept", R.drawable.ic_trash_2, chevron = false) {
                actions.confirm("Empty the cache?", "Songs you have played will download again next time. Your downloads are kept.", "Empty") {
                    actions.launch {
                        graph.api.clearCache()
                        cacheLoader.refreshBehind()
                        actions.say("Emptied; downloads were kept")
                    }
                }
            }
        })

        add(SettingEntry("Lyrics", "Romanise", "latin script") { Toggle("Romanise", "Show lyrics in other scripts in Latin letters", s.lyricsRomanise) { v -> set { it.copy(lyricsRomanise = v) } } })
        add(SettingEntry("Lyrics", "Translate", "language google") { Toggle("Translate", "Show a translation into your language (sent to Google Translate)", s.lyricsTranslate) { v -> set { it.copy(lyricsTranslate = v) } } })

        add(SettingEntry("Your listening", "Listening stats", "top songs artists hours") { Action("Listening stats", "Your top songs, artists and hours", R.drawable.ic_audio_lines) { actions.go(Routes.STATS) } })
        add(SettingEntry("Your listening", "Your year", "recap wrapped") { Action("Your year", "Your year in music, as a story", R.drawable.ic_gift) { actions.go(Routes.RECAP) } })
        add(SettingEntry("Your listening", "New releases", "alerts notifications follow") { Toggle("New releases", "Tell me when an artist I follow releases something", s.newReleaseAlerts) { v -> set { it.copy(newReleaseAlerts = v) } } })
        add(SettingEntry("Your listening", "Import playlists", "spotify apple music deezer export file csv") { Action("Import playlists", "From Spotify, Apple Music or Deezer, by link or export file", R.drawable.ic_list_plus) { actions.go(Routes.IMPORT) } })
        add(SettingEntry("Your listening", "Listen Together", "room friends") { Action("Listen Together", "Play in step with friends", R.drawable.ic_users) { actions.go(Routes.TOGETHER) } })

        add(SettingEntry("Battery", "Background playback", "optimisation optimization battery stops") {
            val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
            if (pm.isIgnoringBatteryOptimizations(context.packageName)) {
                Line("Background playback", "Battery optimisation is off for this app, so music keeps playing")
            } else {
                Action("Let it play in the background", "Battery optimisation can stop music on some phones", R.drawable.ic_headphones) {
                    @SuppressLint("BatteryLife")
                    val intent = Intent(SystemSettings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${context.packageName}"))
                    runCatching { context.startActivity(intent) }
                }
            }
        })
        add(SettingEntry("Battery", "Why music stops on some phones", "dontkillmyapp") {
            Action("Why music stops on some phones", "dontkillmyapp.com", R.drawable.ic_external_link) {
                runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("https://dontkillmyapp.com"))) }
            }
        })

        add(SettingEntry("About", "Version") { Line("Version", "${BuildConfig.VERSION_NAME}. Not affiliated with Spotify or YouTube.") })
        add(SettingEntry("About", "Check for updates", "new version install") {
            val (title, subtitle) = updateRow(update)
            Action(title, subtitle, R.drawable.ic_refresh_cw, chevron = false, badge = update is Updater.Status.Available) {
                if (Updater.busy) return@Action
                graph.scope.launch {
                    // What comes of it shows on this row; nothing more needs saying.
                    if (update is Updater.Status.Available) runCatching { Updater.check(context, graph.http, {}) } else Updater.look(graph.http)
                }
            }
        })
        add(SettingEntry("About", "Look for updates by itself", "automatic badge") { Toggle("Look for updates by itself", "Once a day, with a dot when there is one. Nothing is installed without asking", s.checkForUpdates) { v -> set { it.copy(checkForUpdates = v) } } })
        add(SettingEntry("About", "yt-dlp", "update resolver") { Line("yt-dlp", "It comes with the app here, and updates with it.") })
        add(SettingEntry("About", "What's new", "changelog release notes") { Action("What's new", "What changed in each version", R.drawable.ic_list) { actions.go(Routes.WHATS_NEW) } })
        add(SettingEntry("About", "Report a problem", "bug logs diagnostics send") {
            Action("Report a problem", "Makes a file with the app's log and a summary, for you to send. Your sign-in and email are left out", R.drawable.ic_circle_alert, chevron = false) {
                busy = "Making the report…"
                graph.scope.launch {
                    runCatching { context.startActivity(Report.shareIntent(context)) }.onFailure { actions.say("The report could not be made.") }
                    busy = null
                }
            }
        })
        add(SettingEntry("About", "Send the log", "logs export") {
            Action("Send the log", "Only the log, with your sign-in and email left out", R.drawable.ic_share_2, chevron = false) {
                graph.scope.launch {
                    runCatching { context.startActivity(Report.logIntent(context)) }.onFailure { actions.say("The log could not be read.") }
                }
            }
        })
        add(SettingEntry("About", "Licences", "open source notices credits") { Action("Licences", "What the app is made with", R.drawable.ic_info) { actions.go(Routes.LICENCES) } })
        add(SettingEntry("About", "Reset settings", "defaults restore") {
            Action("Reset settings", "Back to how a new install has them. Your accounts, downloads and blocked list stay", R.drawable.ic_rotate_ccw, chevron = false) {
                actions.confirm("Reset settings?", "Every setting goes back to its default. Your accounts, downloads, blocked list and Listen Together servers stay.", "Reset") {
                    actions.write(done = "Settings reset") { graph.settings.update { it.resetKeepingYours() } }
                }
            }
        })
    }

    // Spotify's way: a short list of sections, each opening on its own.
    var section by rememberSaveable { mutableStateOf<String?>(null) }
    var query by rememberSaveable { mutableStateOf("") }
    val back = predictiveBack(section != null || query.isNotEmpty()) { if (section != null) section = null else query = "" }
    // Held here, out of the branches below, so that the first page is where it was left when a section closes.
    val rootList = rememberLazyListState()
    Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.statusBars)) {
        // Out of the list, so that the way back never scrolls away.
        Row(Modifier.padding(end = 16.dp, top = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            GlyphButton(R.drawable.ic_chevron_left, "Back", { if (section != null) section = null else actions.nav.popBackStack() })
            Text(section ?: "Settings", style = Type.bodyStrong.copy(fontSize = 17.sp), modifier = Modifier.semantics { heading() })
        }
        if (section == null) {
            SearchField(query, { query = it }, Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), placeholder = "Search settings")
        }
        val open = section
        val page = Modifier.weight(1f).fillMaxWidth().predictiveBackLayer(back)
        val pad = PaddingValues(bottom = 40.dp)
        when {
            // Each section starts at its top, whatever the page before it was scrolled to.
            open != null -> key(open) {
                LazyColumn(page, rememberLazyListState(), contentPadding = pad) {
                    items(entries.filter { it.section == open }, key = { it.title }) { it.row() }
                }
            }
            query.isNotBlank() -> key("search") {
                val found = entries.filter { settingMatches(query, it.title, it.section, it.words) }
                LazyColumn(page, rememberLazyListState(), contentPadding = pad) {
                    if (found.isEmpty()) item { Line("Nothing matches \"${query.trim()}\"", "Try another word, such as volume or cache.") }
                    items(found, key = { it.section + it.title }) { e ->
                        Column {
                            Text(e.section, style = Type.tiny, color = p.secondary, modifier = Modifier.padding(start = 16.dp, top = 8.dp))
                            e.row()
                        }
                    }
                }
            }
            else -> LazyColumn(page, rootList, contentPadding = pad) {
                item {
                    ProfileCard(account, accountState is Load.Pending, accountState as? Load.Failed, { accountLoader.reload() }) { actions.go(Routes.SIGN_IN) }
                }
                item { AccountList(account != null) }
                items(SECTIONS, key = { it.first }) { (name, about, icon) ->
                    val dot = name == "About" && update is Updater.Status.Available
                    Action(name, if (dot) "A new version is ready" else about, icon, badge = dot) { section = name }
                }
                if (account != null) {
                    item {
                        Box(Modifier.fillMaxWidth().padding(top = 32.dp), contentAlignment = Alignment.Center) {
                            Text(
                                "Log out", style = Type.bodyStrong, color = p.window,
                                modifier = Modifier.clip(RoundedCornerShape(24.dp)).background(p.text)
                                    .pressRow {
                                        actions.confirm(
                                            "Log out of ${account.name.ifEmpty { "this account" }}?",
                                            "Your library and likes stay on your account. You will need to sign in again to see them here.",
                                            "Log out", run = signOut,
                                        )
                                    }.padding(horizontal = 32.dp, vertical = 12.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

/** What the update row says for where an update is. */
private fun updateRow(status: Updater.Status): Pair<String, String?> = when (status) {
    Updater.Status.Idle -> "Check for updates" to null
    Updater.Status.Checking -> "Checking for updates…" to null
    is Updater.Status.Current -> "Check for updates" to "${status.version} is the newest version"
    is Updater.Status.Available -> "Install version ${status.version}" to "Downloads it and asks Android to install it"
    is Updater.Status.Downloading -> "Downloading version ${status.version}" to status.fraction?.let { "${(it * 100).toInt()}%" }
    is Updater.Status.Installing -> "Installing version ${status.version}" to "Android will ask you to confirm"
    is Updater.Status.Failed -> "Check for updates" to status.message
}

/** Who is signed in, picture and name, as Spotify's settings open; or a way to sign in, or why it could not be read. */
@Composable
private fun ProfileCard(account: Account?, loading: Boolean, failed: Load.Failed?, onRetry: () -> Unit, onSignIn: () -> Unit) {
    val p = LocalPalette.current
    val signedOut = account == null && !loading && failed == null
    val problem = failed != null
    Row(
        Modifier.padding(horizontal = 16.dp, vertical = 8.dp).fillMaxWidth().clip(RoundedCornerShape(12.dp))
            .background(p.surface).pressRow(enabled = signedOut || problem) { if (problem) onRetry() else onSignIn() }.padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(56.dp).clip(CircleShape).background(p.surfaceActive), contentAlignment = Alignment.Center) {
            when {
                loading -> CircularProgressIndicator(color = p.accent, strokeWidth = 2.dp, modifier = Modifier.size(24.dp))
                account?.avatarUrl.isNullOrEmpty() -> Glyph(R.drawable.ic_user, null, tint = p.secondary, size = 28.dp)
                else -> AsyncImage(account?.avatarUrl, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            }
        }
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(
                when {
                    loading -> "Loading your account…"
                    problem -> "Couldn't read your account"
                    else -> account?.name?.ifEmpty { "Signed in" } ?: "Not signed in"
                },
                style = Type.bodyStrong.copy(fontSize = 18.sp),
            )
            Text(
                when {
                    loading -> ""
                    problem -> if (failed.message == ApiError.SignedOut.message) "Sign in again" else "Tap to try again"
                    else -> account?.handle?.ifEmpty { "YouTube Music" } ?: "Sign in for your library, likes and Premium audio"
                },
                style = Type.small, color = p.secondary,
            )
        }
        if (signedOut) Glyph(R.drawable.ic_chevron_right, null, tint = p.secondary, size = 20.dp)
    }
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
                    Text(String.format(Locale.US, "%+.1f", live[i] / 100f), style = Type.tiny, color = p.secondary)
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

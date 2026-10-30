package dev.darksavci.spotified.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import dev.darksavci.spotified.api.CoreSettings
import dev.darksavci.spotified.together.Mode
import dev.darksavci.spotified.together.SavedServer
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import java.util.UUID

private val Context.store: DataStore<Preferences> by preferencesDataStore("settings")

enum class Theme { Dark, Black, Light }

/** The app's settings: the device's own, and those the core acts on. */
data class Settings(
    val theme: Theme = Theme.Dark,
    val crossfadeMs: Long = 0,
    val gapless: Boolean = true,
    val normalise: Boolean = true,
    val skipSilence: Boolean = false,
    val speed: Float = 1f,
    val autoplay: Boolean = true,
    val resumeOnLaunch: Boolean = true,
    val reportToYouTube: Boolean = false,
    val cacheMaxMb: Long = 2048,
    val downloadOnWifiOnly: Boolean = true,
    val equalizerOn: Boolean = false,
    /** Millibels per band, comma separated; empty is flat. */
    val equalizerBands: String = "",
    val lyricsTranslate: Boolean = false,
    val lyricsRomanise: Boolean = false,
    val nowPlayingStyle: Int = 0,
    val newReleaseAlerts: Boolean = true,
    val sponsorBlock: Boolean = false,
    /** "high", "standard" or "low", on Wi-Fi and on mobile data. */
    val wifiQuality: String = "high",
    val mobileQuality: String = "high",
    /** "quiet", "normal" or "loud": how loud evening out leaves tracks, as on the desktop. */
    val volumeLevel: String = "normal",
    /** Louder than the system's volume alone gives, as the desktop's boost: 6 dB through the phone's limiter. */
    val volumeBoost: Boolean = false,
    val bassBoost: Boolean = false,
    /** The Listen Together relays saved, and the one rooms are made and joined on. */
    val togetherServers: List<SavedServer> = emptyList(),
    val togetherSelected: String = "",
    /** The name a room sees; empty for the account's. */
    val togetherName: String = "",
    /** Remembered for the next room made. */
    val togetherRoomName: String = "",
    val togetherMode: Mode = Mode.Collaborative,
    val togetherSharePicture: Boolean = false,
    val togetherNotifications: Boolean = false,
    /** What is never to be played; see [Blocked]. */
    val blocked: Blocked = Blocked(),
    /** The time at the seek bar's right end: what is left, or the song's whole length. */
    val seekShowsRemaining: Boolean = true,
    /** Do without page animation, as on the desktop. */
    val reduceMotion: Boolean = false,
    /** At launch, pick up the queue the account has on another device. Off by default: it replaces the queue kept here, and costs a request on every launch. */
    val continueFromYouTubeMusic: Boolean = false,
    /** Look for a new version once a day, and show a badge when there is one. Nothing is downloaded without asking. */
    val checkForUpdates: Boolean = true,
    /** Bars that move with the music on Now Playing. */
    val visualizer: Boolean = false,
) {
    val togetherServer: SavedServer? get() = togetherServers.firstOrNull { it.id == togetherSelected }

    fun forCore(metered: Boolean) = CoreSettings(
        audioQuality = if (metered) mobileQuality else wifiQuality,
        metered = metered,
        crossfadeMs = crossfadeMs,
        gapless = gapless,
        resumeOnLaunch = resumeOnLaunch,
        reportToYouTube = reportToYouTube,
        cacheMaxMb = cacheMaxMb,
        autoplay = autoplay,
        blockedTracks = blocked.coreTracks,
        blockedArtists = blocked.coreArtists,
        blockedAlbums = blocked.coreAlbums,
    )
}

/**
 * The mobile data quality from what is stored. Every save writes every field,
 * so a stored "standard" is usually the old default written back by a change to
 * something else; it counts as chosen only once [chosen] says so. "low" was
 * never a default, so someone picked it.
 */
fun mobileQualityOf(stored: String?, chosen: Boolean, default: String): String = when {
    stored == null -> default
    !chosen && stored == "standard" -> default
    else -> stored
}

class SettingsStore(private val context: Context) {
    private object K {
        val theme = stringPreferencesKey("theme")
        val crossfade = longPreferencesKey("crossfadeMs")
        val gapless = booleanPreferencesKey("gapless")
        val normalise = booleanPreferencesKey("normalise")
        val skipSilence = booleanPreferencesKey("skipSilence")
        val speed = floatPreferencesKey("speed")
        val autoplay = booleanPreferencesKey("autoplay")
        val resume = booleanPreferencesKey("resumeOnLaunch")
        val report = booleanPreferencesKey("reportToYouTube")
        val cacheMax = longPreferencesKey("cacheMaxMb")
        val wifiOnly = booleanPreferencesKey("downloadOnWifiOnly")
        val eqOn = booleanPreferencesKey("equalizerOn")
        val eqBands = stringPreferencesKey("equalizerBands")
        val translate = booleanPreferencesKey("lyricsTranslate")
        val romanise = booleanPreferencesKey("lyricsRomanise")
        val style = intPreferencesKey("nowPlayingStyle")
        val releaseAlerts = booleanPreferencesKey("newReleaseAlerts")
        val sponsorBlock = booleanPreferencesKey("sponsorBlock")
        val wifiQuality = stringPreferencesKey("wifiQuality")
        val mobileQuality = stringPreferencesKey("mobileQuality")
        val mobileQualityChosen = booleanPreferencesKey("mobileQualityChosen")
        val volumeLevel = stringPreferencesKey("volumeLevel")
        val volumeBoost = booleanPreferencesKey("volumeBoost")
        val bassBoost = booleanPreferencesKey("bassBoost")
        val deviceId = stringPreferencesKey("deviceId")
        val offsets = stringPreferencesKey("lyricsOffsets")
        val togetherServers = stringPreferencesKey("togetherServers")
        val togetherSelected = stringPreferencesKey("togetherSelected")
        val togetherName = stringPreferencesKey("togetherName")
        val togetherRoomName = stringPreferencesKey("togetherRoomName")
        val togetherMode = stringPreferencesKey("togetherMode")
        val togetherSharePicture = booleanPreferencesKey("togetherSharePicture")
        val togetherNotifications = booleanPreferencesKey("togetherNotifications")
        val blocked = stringPreferencesKey("blocked")
        val seekShowsRemaining = booleanPreferencesKey("seekShowsRemaining")
        val reduceMotion = booleanPreferencesKey("reduceMotion")
        val continueFromYouTubeMusic = booleanPreferencesKey("continueFromYouTubeMusic")
        val checkForUpdates = booleanPreferencesKey("checkForUpdates")
        val visualizer = booleanPreferencesKey("visualizer")
    }

    private val servers = ListSerializer(SavedServer.serializer())
    private val json = Json { ignoreUnknownKeys = true }

    val settings: Flow<Settings> = context.store.data.map(::read)

    private fun read(p: Preferences): Settings {
        val d = Settings()
        return Settings(
            theme = p[K.theme]?.let { runCatching { Theme.valueOf(it) }.getOrNull() } ?: d.theme,
            crossfadeMs = p[K.crossfade] ?: d.crossfadeMs,
            gapless = p[K.gapless] ?: d.gapless,
            normalise = p[K.normalise] ?: d.normalise,
            skipSilence = p[K.skipSilence] ?: d.skipSilence,
            speed = p[K.speed] ?: d.speed,
            autoplay = p[K.autoplay] ?: d.autoplay,
            resumeOnLaunch = p[K.resume] ?: d.resumeOnLaunch,
            reportToYouTube = p[K.report] ?: d.reportToYouTube,
            cacheMaxMb = p[K.cacheMax] ?: d.cacheMaxMb,
            downloadOnWifiOnly = p[K.wifiOnly] ?: d.downloadOnWifiOnly,
            equalizerOn = p[K.eqOn] ?: d.equalizerOn,
            equalizerBands = p[K.eqBands] ?: d.equalizerBands,
            lyricsTranslate = p[K.translate] ?: d.lyricsTranslate,
            lyricsRomanise = p[K.romanise] ?: d.lyricsRomanise,
            nowPlayingStyle = p[K.style] ?: d.nowPlayingStyle,
            newReleaseAlerts = p[K.releaseAlerts] ?: d.newReleaseAlerts,
            sponsorBlock = p[K.sponsorBlock] ?: d.sponsorBlock,
            wifiQuality = p[K.wifiQuality] ?: d.wifiQuality,
            mobileQuality = mobileQualityOf(p[K.mobileQuality], p[K.mobileQualityChosen] == true, d.mobileQuality),
            volumeLevel = p[K.volumeLevel] ?: d.volumeLevel,
            volumeBoost = p[K.volumeBoost] ?: d.volumeBoost,
            bassBoost = p[K.bassBoost] ?: d.bassBoost,
            togetherServers = p[K.togetherServers]?.let { runCatching { json.decodeFromString(servers, it) }.getOrNull() } ?: d.togetherServers,
            togetherSelected = p[K.togetherSelected] ?: d.togetherSelected,
            togetherName = p[K.togetherName] ?: d.togetherName,
            togetherRoomName = p[K.togetherRoomName] ?: d.togetherRoomName,
            togetherMode = p[K.togetherMode]?.let(Mode::ofWire) ?: d.togetherMode,
            togetherSharePicture = p[K.togetherSharePicture] ?: d.togetherSharePicture,
            togetherNotifications = p[K.togetherNotifications] ?: d.togetherNotifications,
            blocked = p[K.blocked]?.let { runCatching { json.decodeFromString(Blocked.serializer(), it) }.getOrNull() } ?: d.blocked,
            seekShowsRemaining = p[K.seekShowsRemaining] ?: d.seekShowsRemaining,
            reduceMotion = p[K.reduceMotion] ?: d.reduceMotion,
            continueFromYouTubeMusic = p[K.continueFromYouTubeMusic] ?: d.continueFromYouTubeMusic,
            checkForUpdates = p[K.checkForUpdates] ?: d.checkForUpdates,
            visualizer = p[K.visualizer] ?: d.visualizer,
        )
    }

    suspend fun current(): Settings = settings.first()

    suspend fun update(change: (Settings) -> Settings) {
        context.store.edit { p ->
            val before = read(p)
            val s = change(before)
            if (s.mobileQuality != before.mobileQuality) p[K.mobileQualityChosen] = true
            p[K.theme] = s.theme.name
            p[K.crossfade] = s.crossfadeMs
            p[K.gapless] = s.gapless
            p[K.normalise] = s.normalise
            p[K.skipSilence] = s.skipSilence
            p[K.speed] = s.speed
            p[K.autoplay] = s.autoplay
            p[K.resume] = s.resumeOnLaunch
            p[K.report] = s.reportToYouTube
            p[K.cacheMax] = s.cacheMaxMb
            p[K.wifiOnly] = s.downloadOnWifiOnly
            p[K.eqOn] = s.equalizerOn
            p[K.eqBands] = s.equalizerBands
            p[K.translate] = s.lyricsTranslate
            p[K.romanise] = s.lyricsRomanise
            p[K.style] = s.nowPlayingStyle
            p[K.releaseAlerts] = s.newReleaseAlerts
            p[K.sponsorBlock] = s.sponsorBlock
            p[K.wifiQuality] = s.wifiQuality
            p[K.mobileQuality] = s.mobileQuality
            p[K.volumeLevel] = s.volumeLevel
            p[K.volumeBoost] = s.volumeBoost
            p[K.bassBoost] = s.bassBoost
            p[K.togetherServers] = json.encodeToString(servers, s.togetherServers)
            p[K.togetherSelected] = s.togetherSelected
            p[K.togetherName] = s.togetherName
            p[K.togetherRoomName] = s.togetherRoomName
            p[K.togetherMode] = s.togetherMode.wire
            p[K.togetherSharePicture] = s.togetherSharePicture
            p[K.togetherNotifications] = s.togetherNotifications
            p[K.blocked] = json.encodeToString(Blocked.serializer(), s.blocked)
            p[K.seekShowsRemaining] = s.seekShowsRemaining
            p[K.reduceMotion] = s.reduceMotion
            p[K.continueFromYouTubeMusic] = s.continueFromYouTubeMusic
            p[K.checkForUpdates] = s.checkForUpdates
            p[K.visualizer] = s.visualizer
        }
    }

    /** This device's id with the core, made once and kept. */
    suspend fun deviceId(): String {
        context.store.data.first()[K.deviceId]?.let { return it }
        val id = "android-" + UUID.randomUUID()
        context.store.edit { it[K.deviceId] = id }
        return id
    }

    /** The lyrics timing offset of each song, in milliseconds. */
    val lyricsOffsets: Flow<Map<String, Long>> = context.store.data.map { p ->
        p[K.offsets].orEmpty().split(';').mapNotNull { pair ->
            val (id, ms) = pair.split('=').takeIf { it.size == 2 } ?: return@mapNotNull null
            ms.toLongOrNull()?.let { id to it }
        }.toMap()
    }

    suspend fun setLyricsOffset(trackId: String, ms: Long) {
        context.store.edit { p ->
            val map = p[K.offsets].orEmpty().split(';').mapNotNull { pair ->
                pair.split('=').takeIf { it.size == 2 }?.let { it[0] to it[1] }
            }.toMap().toMutableMap()
            if (ms == 0L) map.remove(trackId) else map[trackId] = ms.toString()
            p[K.offsets] = map.entries.joinToString(";") { "${it.key}=${it.value}" }
        }
    }
}

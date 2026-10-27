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
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
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
    val lastfmSession: String = "",
    val lastfmUser: String = "",
    val lastfmKey: String = "",
    val lastfmSecret: String = "",
    /** A token Last.fm handed out, waiting for the listener to allow the app. */
    val lastfmToken: String = "",
    val newReleaseAlerts: Boolean = true,
    val sponsorBlock: Boolean = false,
) {
    fun forCore() = CoreSettings(
        crossfadeMs = crossfadeMs,
        gapless = gapless,
        resumeOnLaunch = resumeOnLaunch,
        reportToYouTube = reportToYouTube,
        cacheMaxMb = cacheMaxMb,
        autoplay = autoplay,
    )
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
        val lastfmSession = stringPreferencesKey("lastfmSession")
        val lastfmUser = stringPreferencesKey("lastfmUser")
        val lastfmKey = stringPreferencesKey("lastfmKey")
        val lastfmSecret = stringPreferencesKey("lastfmSecret")
        val lastfmToken = stringPreferencesKey("lastfmToken")
        val releaseAlerts = booleanPreferencesKey("newReleaseAlerts")
        val sponsorBlock = booleanPreferencesKey("sponsorBlock")
        val deviceId = stringPreferencesKey("deviceId")
        val offsets = stringPreferencesKey("lyricsOffsets")
    }

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
            lastfmSession = p[K.lastfmSession] ?: d.lastfmSession,
            lastfmUser = p[K.lastfmUser] ?: d.lastfmUser,
            lastfmKey = p[K.lastfmKey] ?: d.lastfmKey,
            lastfmSecret = p[K.lastfmSecret] ?: d.lastfmSecret,
            lastfmToken = p[K.lastfmToken] ?: d.lastfmToken,
            newReleaseAlerts = p[K.releaseAlerts] ?: d.newReleaseAlerts,
            sponsorBlock = p[K.sponsorBlock] ?: d.sponsorBlock,
        )
    }

    suspend fun current(): Settings = settings.first()

    suspend fun update(change: (Settings) -> Settings) {
        context.store.edit { p ->
            val s = change(read(p))
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
            p[K.lastfmSession] = s.lastfmSession
            p[K.lastfmUser] = s.lastfmUser
            p[K.lastfmKey] = s.lastfmKey
            p[K.lastfmSecret] = s.lastfmSecret
            p[K.lastfmToken] = s.lastfmToken
            p[K.releaseAlerts] = s.newReleaseAlerts
            p[K.sponsorBlock] = s.sponsorBlock
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

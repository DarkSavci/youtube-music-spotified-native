package dev.darksavci.spotified.ui.screens

import android.content.Context
import android.content.SharedPreferences
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import dev.darksavci.spotified.api.CoreJson
import dev.darksavci.spotified.api.Projection
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.MapSerializer
import kotlinx.serialization.builtins.serializer

/**
 * What the browsing pages remember between launches: how the library is shown,
 * recent searches, and whether this country has a podcasts page. Read at once
 * rather than through a flow, so that a page does not open one way and flip.
 */
class PagePrefs private constructor(private val prefs: SharedPreferences) {
    var libraryGrid by mutableStateOf(prefs.getBoolean(GRID, false))
        private set
    var librarySort by mutableStateOf(
        runCatching { LibrarySort.valueOf(prefs.getString(SORT, null).orEmpty()) }.getOrDefault(LibrarySort.Recent),
    )
        private set
    /** Off, as the desktop has it. */
    var showMusicVideos by mutableStateOf(prefs.getBoolean(VIDEOS, false))
        private set
    var recentSearches by mutableStateOf(readList())
        private set

    fun setGrid(on: Boolean) {
        libraryGrid = on
        prefs.edit().putBoolean(GRID, on).apply()
    }

    fun setSort(sort: LibrarySort) {
        librarySort = sort
        prefs.edit().putString(SORT, sort.name).apply()
    }

    fun setVideos(on: Boolean) {
        showMusicVideos = on
        prefs.edit().putBoolean(VIDEOS, on).apply()
    }

    fun rememberSearch(query: String) = saveSearches(rememberedSearch(recentSearches, query))

    fun forgetSearch(query: String) = saveSearches(forgottenSearch(recentSearches, query))

    fun clearSearches() = saveSearches(emptyList())

    private fun saveSearches(list: List<String>) {
        if (list == recentSearches) return
        recentSearches = list
        prefs.edit().putString(SEARCHES, CoreJson.encodeToString(ListSerializer(String.serializer()), list)).apply()
    }

    private fun readList(): List<String> =
        prefs.getString(SEARCHES, null)
            ?.let { runCatching { CoreJson.decodeFromString(ListSerializer(String.serializer()), it) }.getOrNull() }
            .orEmpty()

    /** The answer kept about the podcasts page, while it is young enough to trust. Null: ask. */
    fun podcastsPage(now: Long = System.currentTimeMillis()): Boolean? {
        if (!prefs.contains(PODCASTS) || now - prefs.getLong(PODCASTS_AT, 0) > PODCASTS_KEPT_MS) return null
        return prefs.getBoolean(PODCASTS, false)
    }

    fun setPodcastsPage(has: Boolean, now: Long = System.currentTimeMillis()) {
        prefs.edit().putBoolean(PODCASTS, has).putLong(PODCASTS_AT, now).apply()
    }

    companion object {
        private const val GRID = "libraryGrid"
        private const val SORT = "librarySort"
        private const val VIDEOS = "showMusicVideos"
        private const val SEARCHES = "recentSearches"
        private const val PODCASTS = "podcastsPage"
        private const val PODCASTS_AT = "podcastsPageAt"
        private const val PODCASTS_KEPT_MS = 7 * 24 * 60 * 60 * 1000L

        @Volatile
        private var made: PagePrefs? = null

        fun of(context: Context): PagePrefs = made ?: synchronized(this) {
            made ?: PagePrefs(context.applicationContext.getSharedPreferences("pages", Context.MODE_PRIVATE)).also { made = it }
        }
    }
}

/**
 * How far into a long track the listener got, kept for the podcast pages. Only
 * long tracks, which is what episodes are; the core does not say which are which.
 */
object EpisodeProgress {
    private const val LONG_MS = 10 * 60 * 1000L
    private const val SAVE_EVERY_MS = 10_000L
    private const val KEPT = 200
    private const val FILE = "episode-progress"
    private const val KEY = "positions"

    private val serializer = MapSerializer(String.serializer(), Long.serializer())
    private var positions: MutableMap<String, Long>? = null
    private var prefs: SharedPreferences? = null

    private fun loaded(context: Context): MutableMap<String, Long> = synchronized(this) {
        positions ?: run {
            val store = context.applicationContext.getSharedPreferences(FILE, Context.MODE_PRIVATE).also { prefs = it }
            val read = store.getString(KEY, null)?.let { runCatching { CoreJson.decodeFromString(serializer, it) }.getOrNull() }.orEmpty()
            LinkedHashMap(read).also { positions = it }
        }
    }

    fun of(context: Context, id: String): Long = synchronized(this) { loaded(context)[id] ?: 0L }

    /** Follows what plays for as long as it is collected. */
    suspend fun follow(context: Context, projection: Flow<Projection>) {
        var savedAt = 0L
        var savedId = ""
        projection
            .map { Triple(it.state.current?.id.orEmpty(), it.state.current?.durationMs ?: 0L, it.state.positionMs) }
            .distinctUntilChanged()
            .collect { (id, duration, position) ->
                if (id.isEmpty() || duration < LONG_MS || position <= 0) return@collect
                val now = System.currentTimeMillis()
                if (id == savedId && now - savedAt < SAVE_EVERY_MS) return@collect
                savedAt = now
                savedId = id
                save(context, id, position)
            }
    }

    private fun save(context: Context, id: String, position: Long) = synchronized(this) {
        val map = loaded(context)
        map.remove(id)
        map[id] = position
        while (map.size > KEPT) map.remove(map.keys.first())
        prefs?.edit()?.putString(KEY, CoreJson.encodeToString(serializer, map))?.apply()
    }
}

package dev.darksavci.spotified.data

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import dev.darksavci.spotified.api.Api
import dev.darksavci.spotified.api.CoreJson
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.graph
import java.io.File
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject

@Serializable
private data class Kept(val id: String = "", val have: Long = 0, val size: Long = 0, val complete: Boolean = false)

@Serializable
private data class Saved(val tracks: List<Track> = emptyList(), val playlists: List<String> = emptyList())

/**
 * Songs kept for listening offline. The core holds the audio, marked so it is
 * never evicted; this keeps what each song is, to list them, and which
 * playlists are kept in step: a new song in one is downloaded at the next
 * sync, on Wi-Fi unless the listener says otherwise.
 */
class Downloads(private val context: Context, private val api: Api, private val scope: CoroutineScope) {
    private val file = File(context.filesDir, "downloads.json")
    private var saved = Saved()

    private val _tracks = MutableStateFlow<List<Track>>(emptyList())
    /** Every download, newest first. */
    val tracks: StateFlow<List<Track>> = _tracks

    private val _done = MutableStateFlow<Set<String>>(emptySet())
    /** The ids of downloads that are whole on the phone. */
    val done: StateFlow<Set<String>> = _done

    private val _progress = MutableStateFlow<Map<String, Float>>(emptyMap())
    val progress: StateFlow<Map<String, Float>> = _progress

    init {
        scope.launch(Dispatchers.IO) {
            saved = runCatching { CoreJson.decodeFromString(Saved.serializer(), file.readText()) }.getOrDefault(Saved())
            _tracks.value = saved.tracks
            watch()
        }
    }

    fun add(tracks: List<Track>) {
        val fresh = tracks.filter { t -> t.playable && t.id.isNotEmpty() && saved.tracks.none { it.id == t.id } }
        if (fresh.isEmpty()) return
        scope.launch {
            update { it.copy(tracks = fresh + it.tracks) }
            fresh.forEach { runCatching { api.post("downloads", it.id, body = JsonObject(emptyMap())) } }
        }
    }

    fun remove(tracks: List<Track>) {
        val ids = tracks.map { it.id }.toSet()
        scope.launch {
            update { s -> s.copy(tracks = s.tracks.filter { it.id !in ids }) }
            ids.forEach { runCatching { api.call<Unit>(null, "DELETE", null, "downloads", it) } }
        }
    }

    /** Downloads a playlist and keeps it in step from now on. */
    suspend fun addPlaylist(id: String) {
        update { s -> s.copy(playlists = (s.playlists + id).distinct()) }
        add(api.completePlaylist(id).tracks)
        schedule(context)
    }

    suspend fun stopSyncing(id: String) = update { s -> s.copy(playlists = s.playlists - id) }

    val syncedPlaylists: List<String> get() = saved.playlists

    /** Adds what the kept playlists have gained since. */
    suspend fun sync() {
        for (id in saved.playlists) runCatching { add(api.completePlaylist(id).tracks) }
    }

    private suspend fun update(change: (Saved) -> Saved) = withContext(Dispatchers.IO) {
        synchronized(this@Downloads) {
            saved = change(saved)
            _tracks.value = saved.tracks
            val tmp = File(file.path + ".part")
            tmp.writeText(CoreJson.encodeToString(Saved.serializer(), saved))
            tmp.renameTo(file)
        }
    }

    /** Follows the core's progress while anything is still coming. */
    private suspend fun watch() {
        while (true) {
            val kept = runCatching { api.getList(Kept.serializer(), "downloads") }.getOrNull()
            if (kept != null) {
                _done.value = kept.filter { it.complete }.map { it.id }.toSet()
                _progress.value = kept.filter { !it.complete && it.size > 0 }.associate { it.id to it.have.toFloat() / it.size }
                // A song the core has no mark for (its data was cleared) is asked for again.
                val marked = kept.map { it.id }.toSet()
                saved.tracks.filter { it.id !in marked }.forEach {
                    runCatching { api.post("downloads", it.id, body = JsonObject(emptyMap())) }
                }
            }
            delay(if (_progress.value.isEmpty()) 15_000 else 2_000)
        }
    }

    companion object {
        private const val WORK = "sync-downloads"

        /** The periodic sync of kept playlists, on the network the listener allows. */
        fun schedule(context: Context, wifiOnly: Boolean = true) {
            val constraints = Constraints.Builder()
                .setRequiredNetworkType(if (wifiOnly) NetworkType.UNMETERED else NetworkType.CONNECTED)
                .setRequiresBatteryNotLow(true)
                .build()
            val work = PeriodicWorkRequestBuilder<SyncWorker>(12, TimeUnit.HOURS).setConstraints(constraints).build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(WORK, ExistingPeriodicWorkPolicy.UPDATE, work)
        }
    }
}

class SyncWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        applicationContext.graph.core.running()
        applicationContext.graph.downloads.sync()
        return Result.success()
    }
}

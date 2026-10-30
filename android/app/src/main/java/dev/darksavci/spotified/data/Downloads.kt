package dev.darksavci.spotified.data

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import dev.darksavci.spotified.api.Api
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.CoreJson
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.graph
import java.io.File
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject

/** A download as the core has it. */
@Serializable
data class KeptSong(val id: String = "", val have: Long = 0, val size: Long = 0, val complete: Boolean = false)

@Serializable
private data class Saved(val tracks: List<Track> = emptyList(), val playlists: List<String> = emptyList(), val paused: Boolean = false)

/** Where a download is. */
sealed interface DownloadStatus {
    object Done : DownloadStatus
    /** Coming now; [fraction] once its size is known. */
    data class Active(val fraction: Float?) : DownloadStatus
    /** In line behind the ones coming now. */
    data class Waiting(val fraction: Float?) : DownloadStatus
    data class Paused(val fraction: Float?) : DownloadStatus
    /** It stopped getting anywhere, and was tried again without luck. */
    data class Failed(val fraction: Float?) : DownloadStatus
}

/**
 * How the songs wanted move through the core, worked out afresh from what it
 * says it has at each look. The core has no queue or failure of its own to ask:
 * a song is asked for, its bytes grow or they don't. So only a few are asked for
 * at a time, which is what makes pausing mean something; one whose bytes stop
 * growing is asked again, and after that shown as failed for the listener to retry.
 */
object DownloadPlan {
    /** Whether transfers wait: the listener wants them on Wi-Fi only, and this network costs. */
    fun heldForWifi(wifiOnly: Boolean, metered: Boolean) = wifiOnly && metered

    /** How many songs are asked of the core at once. */
    const val AT_ONCE = 3

    /** How long a song may get nowhere before it counts as stuck. */
    const val STUCK_MS = 120_000L

    /** How many times a song is asked for before it counts as failed. */
    const val TRIES = 2

    /** A song asked for: how much it had when it last grew, and when that was. */
    data class Asked(val have: Long, val at: Long, val tries: Int = 1)

    /** What is known in this run of the app. It starts empty: a song marked by an earlier run has to be asked again. */
    data class Session(val asked: Map<String, Asked> = emptyMap(), val failed: Map<String, Long> = emptyMap())

    data class Result(val statuses: Map<String, DownloadStatus>, val ask: List<String>, val session: Session)

    fun look(wanted: List<String>, kept: Map<String, KeptSong>, paused: Boolean, session: Session, now: Long, atOnce: Int = AT_ONCE): Result {
        val statuses = LinkedHashMap<String, DownloadStatus>()
        val asked = HashMap<String, Asked>()
        val failed = HashMap<String, Long>()
        val ask = mutableListOf<String>()
        val inLine = mutableListOf<String>()
        fun fraction(id: String) = kept[id]?.takeIf { it.size > 0 }?.let { (it.have.toFloat() / it.size).coerceIn(0f, 1f) }
        for (id in wanted) {
            val k = kept[id]
            val have = k?.have ?: 0
            val stuckAt = session.failed[id]
            val was = session.asked[id]
            when {
                k?.complete == true -> statuses[id] = DownloadStatus.Done
                // A song given up on that grows after all is coming after all.
                stuckAt != null && have <= stuckAt -> {
                    failed[id] = stuckAt
                    statuses[id] = DownloadStatus.Failed(fraction(id))
                }
                paused -> statuses[id] = DownloadStatus.Paused(fraction(id))
                stuckAt != null -> {
                    asked[id] = Asked(have, now)
                    statuses[id] = DownloadStatus.Active(fraction(id))
                }
                was == null -> inLine += id
                have > was.have -> {
                    asked[id] = was.copy(have = have, at = now)
                    statuses[id] = DownloadStatus.Active(fraction(id))
                }
                now - was.at < STUCK_MS -> {
                    asked[id] = was
                    statuses[id] = DownloadStatus.Active(fraction(id))
                }
                was.tries < TRIES -> {
                    asked[id] = Asked(have, now, was.tries + 1)
                    ask += id
                    statuses[id] = DownloadStatus.Active(fraction(id))
                }
                else -> {
                    failed[id] = have
                    statuses[id] = DownloadStatus.Failed(fraction(id))
                }
            }
        }
        var room = atOnce - asked.size
        for (id in inLine) {
            if (room > 0) {
                room--
                asked[id] = Asked(kept[id]?.have ?: 0, now)
                ask += id
                statuses[id] = DownloadStatus.Active(fraction(id))
            } else {
                statuses[id] = DownloadStatus.Waiting(fraction(id))
            }
        }
        return Result(wanted.associateWith { statuses.getValue(it) }, ask, Session(asked, failed))
    }

    /** A song to try afresh: it goes back in line. */
    fun retry(session: Session, id: String) = Session(session.asked - id, session.failed - id)

    /** Stops counting what was asked for, so that a pause holds all of it and a resume asks again. Failed songs stay failed. */
    fun forgetAsked(session: Session) = Session(emptyMap(), session.failed)

    /** How far a batch of songs has come, from 0 to 1: finished ones whole, the rest by what has arrived. */
    fun overall(batch: Collection<String>, statuses: Map<String, DownloadStatus>): Float {
        if (batch.isEmpty()) return 0f
        val total = batch.sumOf { id ->
            when (val s = statuses[id]) {
                DownloadStatus.Done, null -> 1.0
                is DownloadStatus.Active -> (s.fraction ?: 0f).toDouble()
                is DownloadStatus.Waiting -> (s.fraction ?: 0f).toDouble()
                is DownloadStatus.Paused -> (s.fraction ?: 0f).toDouble()
                is DownloadStatus.Failed -> (s.fraction ?: 0f).toDouble()
            }
        }
        return (total / batch.size).toFloat().coerceIn(0f, 1f)
    }
}

/** Songs of one album, for the page's grouping; [key] is empty for those with no album. */
data class DownloadGroup(val key: String, val title: String, val artist: String, val artwork: List<Artwork>, val tracks: List<Track>)

/** Songs gathered by album, in the order their albums were first met; those with no album come last, together. */
fun groupByAlbum(tracks: List<Track>): List<DownloadGroup> {
    val (loose, inAlbum) = tracks.partition { it.album?.name.isNullOrEmpty() }
    val groups = inAlbum.groupBy { it.album?.id?.ifEmpty { null } ?: it.album?.name.orEmpty() }.map { (key, songs) ->
        val first = songs.first()
        DownloadGroup(key, first.album?.name.orEmpty(), first.artists.firstOrNull()?.name.orEmpty(), first.artwork, songs)
    }
    return if (loose.isEmpty()) groups else groups + DownloadGroup("", "Other songs", "", emptyList(), loose)
}

/**
 * Songs kept for listening offline. The core holds the audio, marked so it is
 * never evicted; this keeps what each song is, to list them, and which
 * playlists are kept in step: a new song in one is downloaded at the next
 * sync, on Wi-Fi unless the listener says otherwise.
 */
class Downloads(
    private val context: Context,
    private val api: Api,
    private val scope: CoroutineScope,
    private val settings: SettingsStore,
    metered: Flow<Boolean>,
) {
    private val wifiHold = combine(settings.settings.map { it.downloadOnWifiOnly }, metered, DownloadPlan::heldForWifi).distinctUntilChanged()

    private val file = File(context.filesDir, "downloads.json")
    @Volatile
    private var saved = Saved()
    private var session = DownloadPlan.Session()
    private val lock = Any()
    private val wake = Channel<Unit>(Channel.CONFLATED)
    private val notice = DownloadNotice(context)

    private val _tracks = MutableStateFlow<List<Track>>(emptyList())
    /** Every download, newest first. */
    val tracks: StateFlow<List<Track>> = _tracks

    private val _done = MutableStateFlow<Set<String>>(emptySet())
    /** The ids of downloads that are whole on the phone. */
    val done: StateFlow<Set<String>> = _done

    private val _progress = MutableStateFlow<Map<String, Float>>(emptyMap())
    val progress: StateFlow<Map<String, Float>> = _progress

    private val _statuses = MutableStateFlow<Map<String, DownloadStatus>>(emptyMap())
    /** Where each download is. */
    val statuses: StateFlow<Map<String, DownloadStatus>> = _statuses

    private val _paused = MutableStateFlow(false)
    /** Whether the listener has held the downloads back. */
    val paused: StateFlow<Boolean> = _paused

    private val _bytes = MutableStateFlow(0L)
    /** What the downloads take on the phone so far. */
    val bytes: StateFlow<Long> = _bytes

    private val _waitingForWifi = MutableStateFlow(false)
    /** The listener wants downloads on Wi-Fi only, and the phone is on a network that costs. */
    val waitingForWifi: StateFlow<Boolean> = _waitingForWifi

    private val _unpulled = MutableStateFlow<Set<String>>(emptySet())
    /** Playlists kept in step whose songs have not been asked for yet, for want of Wi-Fi. */
    val unpulled: StateFlow<Set<String>> = _unpulled

    init {
        scope.launch(Dispatchers.IO) {
            saved = runCatching { CoreJson.decodeFromString(Saved.serializer(), file.readText()) }.getOrDefault(Saved())
            _tracks.value = saved.tracks
            _paused.value = saved.paused
            // Known before the first look, so that it asks for nothing it should not.
            _waitingForWifi.value = wifiHold.first()
            launch { wifiHold.drop(1).collect(::wifiHoldChanged) }
            launch {
                settings.settings.map { it.downloadOnWifiOnly }.distinctUntilChanged().drop(1).collect { wifiOnly ->
                    if (saved.playlists.isNotEmpty()) schedule(context, wifiOnly)
                }
            }
            watch()
        }
    }

    fun add(tracks: List<Track>) {
        val fresh = tracks.filter { t -> t.playable && t.id.isNotEmpty() && saved.tracks.none { it.id == t.id } }
        if (fresh.isEmpty()) return
        scope.launch {
            update { it.copy(tracks = fresh + it.tracks) }
            wake.trySend(Unit)
        }
    }

    /** Takes songs off the list, which is also what cancelling one is. */
    fun remove(tracks: List<Track>) {
        val ids = tracks.map { it.id }.toSet()
        scope.launch {
            update { s -> s.copy(tracks = s.tracks.filter { it.id !in ids }) }
            ids.forEach { id ->
                changeSession { DownloadPlan.retry(it, id) }
                runCatching { api.call<Unit>(null, "DELETE", null, "downloads", id) }
            }
            wake.trySend(Unit)
        }
    }

    private fun changeSession(change: (DownloadPlan.Session) -> DownloadPlan.Session) {
        synchronized(lock) { session = change(session) }
    }

    /**
     * Stops what is coming and holds back the rest. Letting a song go is the core's
     * way to stop its transfer; resuming asks for it again, and what arrived is
     * carried on from.
     */
    fun pause() {
        scope.launch {
            update { it.copy(paused = true) }
            _paused.value = true
            letGoOfWhatIsComing()
            wake.trySend(Unit)
        }
    }

    private suspend fun letGoOfWhatIsComing() {
        val coming = synchronized(lock) { session.asked.keys.toList() }
        changeSession(DownloadPlan::forgetAsked)
        coming.forEach { id -> runCatching { api.call<Unit>(null, "DELETE", null, "downloads", id) } }
    }

    /** On a network that costs, transfers stop; back on Wi-Fi, they and the playlists left waiting start. */
    private suspend fun wifiHoldChanged(held: Boolean) {
        _waitingForWifi.value = held
        if (held) {
            letGoOfWhatIsComing()
        } else {
            val waiting = _unpulled.value
            _unpulled.value = emptySet()
            waiting.forEach { id -> runCatching { add(api.completePlaylist(id).tracks) } }
        }
        wake.trySend(Unit)
    }

    fun resume() {
        scope.launch {
            update { it.copy(paused = false) }
            _paused.value = false
            wake.trySend(Unit)
        }
    }

    /** A failed song goes back in line. */
    fun retry(id: String) {
        scope.launch {
            changeSession { DownloadPlan.retry(it, id) }
            wake.trySend(Unit)
        }
    }

    fun retryAll() {
        scope.launch {
            changeSession { s -> s.failed.keys.fold(s) { all, id -> DownloadPlan.retry(all, id) } }
            wake.trySend(Unit)
        }
    }

    /** Looks at once, for a pull on the page: what comes back is said by the flows above. A failure is thrown. */
    suspend fun refresh() {
        applyKept(api.getList(KeptSong.serializer(), "downloads"))
    }

    /** Downloads a playlist and keeps it in step from now on. */
    suspend fun addPlaylist(id: String) {
        update { s -> s.copy(playlists = (s.playlists + id).distinct()) }
        schedule(context, settings.current().downloadOnWifiOnly)
        if (_waitingForWifi.value) _unpulled.update { it + id } else add(api.completePlaylist(id).tracks)
    }

    suspend fun stopSyncing(id: String) {
        _unpulled.update { it - id }
        update { s -> s.copy(playlists = s.playlists - id) }
    }

    val syncedPlaylists: List<String> get() = saved.playlists

    /** Adds what the kept playlists have gained since. */
    suspend fun sync() {
        if (_waitingForWifi.value) return
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

    /** Follows the core's progress, looking often while anything is coming. */
    private suspend fun watch() {
        while (true) {
            runCatching { refresh() }
            val under = _statuses.value.values.any { it is DownloadStatus.Active || it is DownloadStatus.Waiting }
            withTimeoutOrNull(if (under) 2_000L else 15_000L) { wake.receive() }
        }
    }

    private suspend fun applyKept(kept: List<KeptSong>) {
        val look = synchronized(lock) {
            DownloadPlan.look(saved.tracks.map { it.id }, kept.associateBy { it.id }, saved.paused || _waitingForWifi.value, session, System.currentTimeMillis())
                .also { session = it.session }
        }
        _done.value = kept.filter { it.complete }.map { it.id }.toSet()
        _progress.value = look.statuses.mapNotNull { (id, s) -> (s as? DownloadStatus.Active)?.fraction?.let { id to it } }.toMap()
        _statuses.value = look.statuses
        _bytes.value = kept.filter { k -> saved.tracks.any { it.id == k.id } }.sumOf { it.have }
        // One the core refused is asked again at the next look, by being let go here.
        look.ask.forEach { id ->
            runCatching { api.post("downloads", id, body = JsonObject(emptyMap())) }
                .onFailure { changeSession { s -> DownloadPlan.retry(s, id) } }
        }
        // Waiting for Wi-Fi is said on the page: there is nothing for a Pause button to do about it.
        synchronized(lock) {
            if (_waitingForWifi.value) notice.show(emptyList(), emptyMap(), false)
            else notice.show(saved.tracks, look.statuses, saved.paused)
        }
    }

    /** Pause or resume, from the notification's buttons. */
    class FromNotice : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val downloads = context.graph.downloads
            when (intent.action) {
                DownloadNotice.ACTION_PAUSE -> downloads.pause()
                DownloadNotice.ACTION_RESUME -> downloads.resume()
            }
        }
    }

    companion object {
        private const val WORK = "sync-downloads"

        /** The periodic sync of kept playlists, on the network the listener allows. */
        fun schedule(context: Context, wifiOnly: Boolean) {
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

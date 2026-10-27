package dev.darksavci.spotified.playback

import android.util.Log
import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.api.EngineEvent
import dev.darksavci.spotified.api.Projection
import dev.darksavci.spotified.api.Repeat
import dev.darksavci.spotified.api.Session
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.SettingsStore
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import okhttp3.HttpUrl

/**
 * The playback session as the app sees it: the core's projection, kept
 * current over its event stream, and the commands sent back. The engine
 * attaches itself here and receives every target; the screens and the media
 * session read [projection] and send through the methods below.
 */
class Playback(
    private val scope: CoroutineScope,
    private val session: Session,
    private val settings: SettingsStore,
    private val origin: suspend () -> HttpUrl,
    /** Whether the phone is on a network that costs, for the quality to ask for. */
    private val metered: Flow<Boolean>,
) {
    private val _projection = MutableStateFlow(Projection())
    val projection: StateFlow<Projection> = _projection

    private val _connected = MutableStateFlow(false)
    val connected: StateFlow<Boolean> = _connected

    private val _messages = MutableSharedFlow<String>(extraBufferCapacity = 8)
    /** Sentences for a toast: a refused command, a lost connection. */
    val messages: SharedFlow<String> = _messages

    @Volatile
    var engine: Engine? = null
        set(value) {
            field = value
            value?.let { e -> scope.launch(Dispatchers.Main) { e.apply(_projection.value.target) } }
        }

    private var deviceId = ""
    private var loop: Job? = null
    private val reports = Mutex()
    /** The first projection after a reconnect is taken whatever its version: a restarted core restarts them. */
    private var acceptAny = true

    /** Connects to the session and keeps it connected. Called once; later calls do nothing. */
    fun start() {
        if (loop != null) return
        loop = scope.launch {
            deviceId = settings.deviceId()
            // The core acts on some settings; every change is sent whole.
            launch {
                combine(settings.settings, metered) { s, m -> s.forCore(m) }.distinctUntilChanged().collect { core ->
                    runCatching { session.setSettings(core) }
                }
            }
            var backoff = 500L
            while (true) {
                try {
                    acceptAny = true
                    accept(session.register(deviceId))
                    _connected.value = true
                    backoff = 500L
                    session.events(deviceId, origin).collect { accept(it) }
                } catch (e: ApiError) {
                    Log.w(TAG, "session: ${e.message}")
                } catch (e: Exception) {
                    if (e is CancellationException) throw e
                    Log.w(TAG, "session", e)
                }
                _connected.value = false
                delay(backoff)
                backoff = (backoff * 2).coerceAtMost(8000)
            }
        }
    }

    /** Takes a projection unless an older one than shown. */
    private suspend fun accept(p: Projection) {
        val shown = _projection.value
        if (!acceptAny && p.state.version < shown.state.version) return
        acceptAny = false
        _projection.value = p
        engine?.let { e -> withContext(Dispatchers.Main) { e.apply(p.target) } }
    }

    /** An engine report, sent in order: the core reads them as a sequence. */
    fun report(event: EngineEvent) {
        scope.launch {
            reports.withLock {
                runCatching { session.engineEvent(deviceId, event) }
                    .onFailure { Log.w(TAG, "engine event ${event.kind}: ${it.message}") }
            }
        }
    }

    /**
     * While in a Listen Together room the core takes only following, leaving
     * and volume from this player; anything else is the room's to decide,
     * and goes there. Returns whether the room took the command.
     */
    @Volatile
    var room: ((Command) -> Boolean)? = null

    fun send(command: Command) {
        val ownsIt = command is Command.FollowRoom || command is Command.LeaveRoom || command is Command.SetVolume
        if (!ownsIt && _projection.value.followingRoom && room?.invoke(command) == true) return
        scope.launch {
            try {
                if (deviceId.isEmpty()) deviceId = settings.deviceId()
                accept(session.command(deviceId, command))
            } catch (e: ApiError) {
                _messages.tryEmit(e.message ?: "")
            }
        }
    }

    fun play(tracks: List<Track>, startIndex: Int, origin: String) {
        val playable = tracks.filter { it.playable && it.id.isNotEmpty() }
        if (playable.isEmpty()) return
        val start = playable.indexOfFirst { it.id == tracks.getOrNull(startIndex)?.id }.coerceAtLeast(0)
        send(Command.Play(playable, start, origin))
    }

    fun shuffle(tracks: List<Track>, origin: String) {
        val playable = tracks.filter { it.playable && it.id.isNotEmpty() }
        if (playable.isEmpty()) return
        send(Command.SetShuffle(true))
        send(Command.Play(playable, playable.indices.random(), origin))
    }

    fun toggle() = send(Command.Toggle)

    /** Pauses if playing; for audio focus and a headset unplugged. */
    fun pause() {
        if (_projection.value.target.playing) send(Command.Toggle)
    }

    fun resume() {
        if (!_projection.value.target.playing) send(Command.Toggle)
    }

    fun next() = send(Command.Next)
    fun previous() = send(Command.Previous)
    fun seek(ms: Long) = send(Command.Seek(ms))
    fun jump(index: Int) = send(Command.Jump(index))
    fun remove(index: Int) = send(Command.Remove(index))
    fun move(from: Int, to: Int) = send(Command.Move(from, to))
    fun setShuffle(on: Boolean) = send(Command.SetShuffle(on))
    fun setRepeat(repeat: Repeat) = send(Command.SetRepeat(repeat))

    fun cycleRepeat() = setRepeat(
        when (_projection.value.state.repeat) {
            Repeat.Off -> Repeat.All
            Repeat.All -> Repeat.One
            Repeat.One -> Repeat.Off
        },
    )

    fun playNext(tracks: List<Track>) {
        val q = _projection.value.state.queue
        send(Command.Enqueue(tracks, if (q.items.isEmpty()) null else q.index + 1))
    }

    fun addToQueue(tracks: List<Track>) = send(Command.Enqueue(tracks, null))

    fun startRadio(track: Track) {
        scope.launch {
            runCatching { session.startRadio(deviceId.ifEmpty { settings.deviceId() }, track) }
                .onFailure { _messages.tryEmit(it.message ?: "") }
        }
    }

    fun startMix(playlistId: String, videoId: String, params: String, origin: String) {
        scope.launch {
            runCatching { session.startMix(deviceId.ifEmpty { settings.deviceId() }, playlistId, videoId, params, origin) }
                .onFailure { _messages.tryEmit(it.message ?: "") }
        }
    }

    fun say(message: String) {
        _messages.tryEmit(message)
    }

    companion object {
        private const val TAG = "Playback"
    }
}

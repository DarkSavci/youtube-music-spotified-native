package dev.darksavci.spotified.playback

import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.api.EngineEvent
import dev.darksavci.spotified.api.Projection
import dev.darksavci.spotified.api.Repeat
import dev.darksavci.spotified.api.Session
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.Logs
import dev.darksavci.spotified.data.SettingsStore
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
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
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
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
    /** The core's projection as it came, which is what the engine follows. */
    private val _projection = MutableStateFlow(Projection())
    private val _shown = MutableStateFlow(Projection())
    private val showing = Any()

    /** The core's projection, but paused while this phone is held. */
    val projection: StateFlow<Projection> = _shown

    private val _hold = MutableStateFlow<Hold?>(null)
    /** This phone gone quiet while its room carries on; null when it is not. */
    val hold: StateFlow<Hold?> = _hold

    /** Puts the player back in step with its room, on the main thread; set where the room is known. */
    @Volatile
    var rejoin: (() -> Unit)? = null

    private val _connected = MutableStateFlow(false)
    val connected: StateFlow<Boolean> = _connected

    private val _messages = MutableSharedFlow<String>(extraBufferCapacity = 8)
    /** Sentences for a toast: a refused command, a lost connection. */
    val messages: SharedFlow<String> = _messages

    @Volatile
    var engine: Engine? = null
        set(value) {
            field = value
            value?.let { e ->
                scope.launch(Dispatchers.Main) {
                    e.setHeld(_hold.value != null)
                    e.apply(_projection.value.target)
                }
            }
        }

    private var deviceId = ""
    private var loop: Job? = null
    private val reports = InOrder(scope) { Logs.w(TAG, "engine event", it) }

    /** Commands go one at a time, each answered before the next: the core reads them as a sequence too. */
    private val commands = InOrder(scope) { e ->
        Logs.w(TAG, "command", e)
        _messages.tryEmit("That could not be done.")
    }
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
                    Logs.w(TAG, "session: ${e.message}")
                } catch (e: Exception) {
                    if (e is CancellationException) throw e
                    Logs.w(TAG, "session", e)
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
        synchronized(showing) {
            _projection.value = p
            _shown.value = if (_hold.value != null) p.heldBack() else p
        }
        engine?.let { e -> withContext(Dispatchers.Main) { e.apply(p.target) } }
    }

    /** An engine report, sent in order: the core reads them as a sequence. */
    fun report(event: EngineEvent) {
        reports.post {
            runCatching { session.engineEvent(deviceId, event) }
                .onFailure { Logs.w(TAG, "engine event ${event.kind}: ${it.message}") }
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
        inTurn {
            try {
                order(command)
            } finally {
                if (command is Command.LeaveRoom) leftWhileHeld()
            }
        }
    }

    private fun inTurn(work: suspend () -> Unit) {
        commands.post {
            try {
                work()
            } catch (e: ApiError) {
                _messages.tryEmit(e.message ?: "")
            }
        }
    }

    /** Sends a command and takes the answer. Only for work already in turn. */
    private suspend fun order(command: Command) {
        if (deviceId.isEmpty()) deviceId = settings.deviceId()
        accept(session.command(deviceId, command))
    }

    /** Waits, for a little, until the commands sent so far have been answered. */
    suspend fun settled() {
        val done = CompletableDeferred<Unit>()
        commands.post { done.complete(Unit) }
        withTimeoutOrNull(SETTLE_MS) { done.await() }
    }

    /** Out of the room, a hold becomes what it is everywhere else: the player paused. */
    private suspend fun leftWhileHeld() {
        if (_hold.value == null) return
        try {
            if (_projection.value.target.playing) order(Command.Toggle)
        } finally {
            setHold(null)
        }
    }

    private fun setHold(hold: Hold?) {
        synchronized(showing) {
            _hold.value = hold
            val p = _projection.value
            _shown.value = if (hold != null) p.heldBack() else p
        }
        engine?.let { e -> scope.launch(Dispatchers.Main) { e.setHeld(hold != null) } }
    }

    /** Ends a hold: the room's place first, and the sound once that has been answered, so it starts in step. */
    private fun endHold() {
        scope.launch(Dispatchers.Main) {
            rejoin?.invoke()
            commands.post { setHold(null) }
        }
    }

    /** Asks for playing or paused, judged when its turn comes: what comes before may already have done it. */
    private fun ask(playing: Boolean) {
        if (_projection.value.followingRoom) {
            // The room's to decide, and it goes there with its own view of the moment.
            if (_projection.value.target.playing != playing) send(Command.Toggle)
            return
        }
        inTurn { if (_projection.value.target.playing != playing) order(Command.Toggle) }
    }

    fun play(tracks: List<Track>, startIndex: Int, origin: String) {
        val playable = tracks.filter { it.playable && it.id.isNotEmpty() }
        if (playable.isEmpty()) return
        val start = playable.indexOfFirst { it.id == tracks.getOrNull(startIndex)?.id }.coerceAtLeast(0)
        send(Command.Play(playable, start, origin))
    }

    /**
     * Plays a list from [positionMs] into its [startIndex]th track: the queue put
     * in place held paused, the seek, then play, each answered before the next
     * goes. A paused deck makes no sound, so nothing is heard from the start of the
     * track on the way. A room decides for itself what plays, and from where.
     */
    fun playFrom(tracks: List<Track>, startIndex: Int, origin: String, positionMs: Long) {
        val playable = tracks.filter { it.playable && it.id.isNotEmpty() }
        val wanted = tracks.getOrNull(startIndex)?.id
        val start = playable.indexOfFirst { it.id == wanted }
        if (start < 0 || _projection.value.followingRoom) return play(tracks, startIndex, origin)
        inTurn {
            if (deviceId.isEmpty()) deviceId = settings.deviceId()
            val placed = session.command(deviceId, Command.Play(playable, start, origin, paused = true))
            accept(placed)
            // Blocked, the queue started on another track: this place is not that one's.
            if (placed.state.current?.id == wanted) accept(session.command(deviceId, Command.Seek(positionMs)))
            accept(session.command(deviceId, Command.Toggle))
        }
    }

    fun shuffle(tracks: List<Track>, origin: String) {
        val playable = tracks.filter { it.playable && it.id.isNotEmpty() }
        if (playable.isEmpty()) return
        send(Command.SetShuffle(true))
        send(Command.Play(playable, playable.indices.random(), origin))
    }

    /** The play button: while held, it is what brings this phone back, not the room's to pause. */
    fun toggle() {
        if (_hold.value != null) endHold() else send(Command.Toggle)
    }

    /**
     * Stops the sound because the system asks, not the listener: focus lost, a
     * headset unplugged, a sleep timer run out, an account being left. Pauses
     * if playing. In a room a pause is the room's, so this phone alone goes quiet.
     */
    fun pause(hold: Hold = Hold.UntilAsked) {
        if (_projection.value.followingRoom) setHold(_hold.value.and(hold)) else ask(false)
    }

    /** The listener's pause from the system's controls. While held, this phone is paused already. */
    fun pressPause() {
        if (_hold.value == null) ask(false)
    }

    fun resume() {
        if (_hold.value != null) endHold() else ask(true)
    }

    /** Audio focus came back after a moment's loss. In a room, it ends only a hold that came from that loss. */
    fun focusBack() {
        val hold = _hold.value
        when {
            hold != null -> if (hold.afterFocusBack() == null) endHold()
            !_projection.value.followingRoom -> ask(true)
        }
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
        private const val SETTLE_MS = 5000L
    }
}

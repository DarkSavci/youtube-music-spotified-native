package dev.darksavci.spotified.together

import dev.darksavci.spotified.api.Api
import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.api.PlayState
import dev.darksavci.spotified.api.Projection
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.data.SettingsStore
import dev.darksavci.spotified.playback.Playback
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import okhttp3.OkHttpClient

/*
 * Listen Together, as the app sees it: entering and leaving a room, hearing
 * the room, and keeping the player with it. Mirrors
 * crates/app/src/actions/together.rs.
 *
 * The relay owns the room. Everyone plays the music through their own
 * account and their own player; what is shared is which song, and where in
 * it. This app sends its controls to the room as commands, hears the room
 * back, and has the core follow it. Everything here runs on the main thread,
 * where the engine's position can be read.
 */

/** How things stand with the room. */
enum class Phase {
    /** Not in a room, nor on the way into one. */
    Idle,
    Connecting,

    /** The leader has yet to let this listener in. */
    Waiting,
    Joined,
    Reconnecting,
}

/** What the page shows of Listen Together. */
data class TogetherState(
    val phase: Phase = Phase.Idle,
    /** The room as last heard, while in one. */
    val room: Room? = null,
    /** This listener's member id in the room. */
    val me: String = "",
    /** Why the last attempt failed, or why the room ended. */
    val error: String? = null,
    /** How this player stands with the room, as the page says it. */
    val standing: Standing = Standing.Joining,
    /** The form that adds or edits a saved server, while it is open. */
    val manage: ServerForm? = null,
    /** The PIN of a room to join, as typed. */
    val pin: String = "",
) {
    /** In a room: seated, even if the line has dropped for a moment. */
    val inRoom: Boolean get() = (phase == Phase.Joined || phase == Phase.Reconnecting) && room != null
}

/** One of the leader's settings, changed. */
sealed interface Setting {
    data class ModeTo(val mode: Mode) : Setting
    data class PolicyTo(val policy: Policy) : Setting

    /** The leader lets each new listener in. */
    data class JoinApproval(val on: Boolean) : Setting

    /** No one new may join. */
    data class Locked(val on: Boolean) : Setting

    /** Requests go straight into the queue. */
    data class AutoAccept(val on: Boolean) : Setting
    data class Duplicates(val on: Boolean) : Setting
    data class VoteSkip(val on: Boolean) : Setting

    fun fields(): JsonObject = buildJsonObject {
        when (val s = this@Setting) {
            is ModeTo -> put("mode", s.mode.wire)
            is PolicyTo -> put("policy", s.policy.wire)
            is JoinApproval -> put("joinApproval", s.on)
            is Locked -> put("locked", s.on)
            is AutoAccept -> put("autoAccept", s.on)
            is Duplicates -> put("duplicates", s.on)
            is VoteSkip -> put("voteSkip", s.on)
        }
    }
}

/** The room's own play, pause and skip buttons. */
enum class Transport { Previous, Toggle, Next }

/**
 * Bringing the music that was playing into a room just made: the songs go
 * in, then the room is taken to where the song was, then it plays. Each
 * step waits for the room to have heard the one before.
 */
private sealed interface Seed {
    object None : Seed

    /** To be offered once the room exists. */
    data class Wanted(val tracks: List<Track>, val positionMs: Long, val playing: Boolean) : Seed

    /** The songs are on their way in. */
    data class Enqueued(val positionMs: Long, val playing: Boolean) : Seed

    /** The room has been taken to the place in the song. */
    data class Sought(val playing: Boolean) : Seed
}

/** How often the room is told again that this player reached a song's end, until the room moves on. */
private const val ENDED_EVERY_MS = 5000L

/** A song this far in or less is not worth taking a new room to. */
private const val SEED_SEEK_FROM_MS = 2000L

class Together(
    private val scope: CoroutineScope,
    private val playback: Playback,
    private val store: SettingsStore,
    private val api: Api,
    private val http: OkHttpClient,
) {
    private val _state = MutableStateFlow(TogetherState())
    val state: StateFlow<TogetherState> = _state

    private var settings = Settings()
    private var line: Connection? = null
    private var ticker: Job? = null
    /** Counts the ways into a room, so one given up on is not finished later. */
    private var attempt = 0

    /** How far the relay's clock is ahead of this device's. */
    private var offsetMs = 0.0
    /** The entry the core was last told to follow. */
    private var appliedEntry: String? = null
    /** When the player was last brought back to the room. */
    private var correctedAt: Long? = null
    private var seed: Seed = Seed.None
    /**
     * This listener made the room, whose queue began as theirs. Leaving,
     * they carry on with it; a guest carries on with the song playing and
     * then their own queue.
     */
    private var created = false
    /** When the room was last told this player reached the song's end. */
    private var endedSent: Long? = null
    /** The entry and song whose measured length the room has been told. */
    private var lengthTold: String? = null
    /** Requests this listener withdrew, whose going is not the leader's no. */
    private val withdrawn = mutableListOf<String>()
    /** Songs the room has had while this listener was in it. */
    private val heard = mutableListOf<String>()

    init {
        scope.launch { store.settings.collect { settings = it } }
    }

    /** The relay's clock now. */
    fun serverNow(): Double = nowMs() + offsetMs

    private fun say(message: String) = playback.say(message)

    private fun refuse(why: String) = _state.update { it.copy(error = why) }

    private fun saveSettings(change: (Settings) -> Settings) {
        scope.launch { store.update(change) }
    }

    // ---------- before a room ----------

    fun typePin(text: String) = _state.update { it.copy(pin = pinFrom(text)) }

    fun typeName(text: String) = saveSettings { it.copy(togetherName = text.take(50)) }

    fun typeRoomName(text: String) = saveSettings { it.copy(togetherRoomName = text.take(80)) }

    fun chooseMode(mode: Mode) = saveSettings { it.copy(togetherMode = mode) }

    fun sharePicture(on: Boolean) = saveSettings { it.copy(togetherSharePicture = on) }

    fun notifications(on: Boolean) = saveSettings { it.copy(togetherNotifications = on) }

    fun dismissError() = _state.update { it.copy(error = null) }

    fun create() = enter(Enter.Create(settings.togetherMode, settings.togetherRoomName.trim()))

    fun join() = enter(Enter.Join(_state.value.pin))

    private fun enter(enter: Enter) {
        if (_state.value.phase != Phase.Idle) return
        // With no server chosen the way in is to save one: the form opens.
        val server = settings.togetherServer
        if (server == null) {
            _state.update { it.copy(manage = ServerForm()) }
            return
        }
        if (!playback.connected.value) return refuse("Wait for the local music service to connect, then try again.")
        val url = checkedAddress(server.url).getOrElse { return refuse(it.message.orEmpty()) }
        if (enter is Enter.Join && enter.pin.length != 8) return refuse("A room's PIN has eight digits.")
        // A room made while music plays starts with that music.
        val projection = playback.projection.value
        val session = projection.state
        seed = if (enter is Enter.Create && session.current != null) {
            val from = session.queue.index.coerceIn(0, session.queue.items.size)
            val end = (from + MOST_TRACKS).coerceAtMost(session.queue.items.size)
            Seed.Wanted(session.queue.items.subList(from, end), livePosition(projection), wantsToPlay(projection))
        } else Seed.None
        created = enter is Enter.Create
        _state.update { it.copy(phase = Phase.Connecting, error = null, manage = null) }
        val mine = ++attempt
        scope.launch(Dispatchers.Main) {
            val account = runCatching { api.account() }.getOrNull()
            if (mine != attempt || _state.value.phase != Phase.Connecting) return@launch
            val name = settings.togetherName.trim().ifEmpty { account?.name?.ifEmpty { null } ?: "Listener" }
            val avatar = account?.avatarUrl?.takeIf { settings.togetherSharePicture }.orEmpty()
            val connection = Connection(Options(url, name, avatar, enter), http)
            line = connection
            connection.start(scope) { event ->
                scope.launch(Dispatchers.Main) { if (line === connection) heard(event) }
            }
            ticker = scope.launch(Dispatchers.Main) {
                while (true) {
                    delay(1000)
                    keepUp()
                }
            }
        }
    }

    /**
     * Leaves the room, or gives up entering one. The song playing carries
     * on; after it, the room's queue for whoever made the room, whose queue
     * it began as, and their own queue from before the room for a guest.
     * [next] is who leads the room after this listener, if they lead it.
     */
    fun leave(next: String? = null, error: String? = null) {
        val wasInRoom = _state.value.inRoom
        val leads = _state.value.room?.leads(_state.value.me) == true
        line?.leave(next?.takeIf { leads && it.isNotEmpty() })
        reset(error)
        if (wasInRoom) playback.send(Command.LeaveRoom(keepQueue = true, thenOwnQueue = !created))
    }

    /** Back to how things were before any room, keeping what was typed. */
    private fun reset(error: String?) {
        attempt++
        line = null
        ticker?.cancel()
        ticker = null
        offsetMs = 0.0
        appliedEntry = null
        correctedAt = null
        seed = Seed.None
        endedSent = null
        lengthTold = null
        withdrawn.clear()
        heard.clear()
        _state.update { TogetherState(error = error, manage = it.manage, pin = it.pin) }
    }

    // ---------- saved servers ----------

    /** The server is not changed under a room, nor on the way into one. */
    private val idle get() = _state.value.phase == Phase.Idle

    fun selectServer(id: String) {
        if (idle) saveSettings { it.copy(togetherSelected = id) }
    }

    /** Opens the form on the chosen server, or on a new one; or closes it. */
    fun toggleManage() {
        if (!idle) return
        _state.update { it.copy(manage = if (it.manage != null) null else ServerForm.on(settings.togetherServer)) }
    }

    fun editServer(server: SavedServer?) {
        if (idle) _state.update { it.copy(manage = ServerForm.on(server)) }
    }

    fun closeManage() = _state.update { it.copy(manage = null) }

    fun serverName(name: String) = _state.update { it.copy(manage = it.manage?.copy(name = name.take(80))) }

    fun serverAddress(url: String) = _state.update { it.copy(manage = it.manage?.copy(url = url.take(300))) }

    /** Empties the form for another server. */
    fun addAnother() = _state.update { it.copy(manage = it.manage?.let { ServerForm() }) }

    fun saveServer() {
        val form = _state.value.manage ?: return
        save(settings.togetherServers, form).fold(
            { (servers, id) ->
                saveSettings { it.copy(togetherServers = servers, togetherSelected = id) }
                _state.update { it.copy(manage = null, error = null) }
            },
            { why -> refuse(why.message.orEmpty()) },
        )
    }

    /** Sees whether the address in the form answers as a relay does. */
    fun testServer() {
        val form = _state.value.manage ?: return
        if (form.check == CHECKING) return
        val url = checkedAddress(form.url).getOrElse { why ->
            _state.update { it.copy(manage = it.manage?.copy(check = why.message.orEmpty())) }
            return
        }
        _state.update { it.copy(manage = it.manage?.copy(check = CHECKING)) }
        scope.launch(Dispatchers.Main) {
            val result = Connection.probe(http, url)
            val said = result.fold({ CHECKED }, { it.message.orEmpty() })
            _state.update { it.copy(manage = it.manage?.copy(check = said)) }
        }
    }

    /** Forgets a saved server. None is then chosen. */
    fun removeServer(id: String) {
        saveSettings { s -> s.copy(togetherServers = s.togetherServers.filter { it.id != id }, togetherSelected = "") }
        _state.update { it.copy(manage = null) }
    }

    // ---------- hearing the room ----------

    private fun heard(event: Event) {
        when (event) {
            Event.Reconnecting -> _state.update { it.copy(phase = Phase.Reconnecting) }
            Event.Waiting -> _state.update { it.copy(phase = Phase.Waiting) }
            is Event.Joined -> _state.update { it.copy(me = event.member, phase = Phase.Joined) }
            is Event.RoomState -> {
                val previous = _state.value.room
                // Equal is let through: who is listening changes without the
                // room's revision changing.
                if (event.room.revision < (previous?.revision ?: 0)) return
                offsetMs = event.offsetMs
                _state.update { it.copy(room = event.room, phase = Phase.Joined) }
                if (previous != null) roomChanged(previous, event.room)
                keepUp()
            }
            is Event.Refused -> say(event.why)
            is Event.Ended -> leave(error = event.reason.ifEmpty { null })
            is Event.Failed -> leave(error = event.why)
        }
    }

    /** The room has gone from [previous] to [room]: say what this listener should hear of it, and note what the room has played. */
    private fun roomChanged(previous: Room, room: Room) {
        rememberHeard(heard, room)
        // Someone sought: follow at once, not as drift that may wait.
        if (jumped(previous, room)) correctedAt = null
        val news = requestNews(previous, room, _state.value.me, withdrawn).toMutableList()
        val latest = room.activity.lastOrNull()
        val happened = latest?.id != previous.activity.lastOrNull()?.id
        if (news.isEmpty() && happened && settings.togetherNotifications && latest != null) news += latest.text
        news.forEach(::say)
    }

    private fun wantsToPlay(p: Projection) =
        p.state.state == PlayState.Playing || p.state.state == PlayState.Loading || p.state.state == PlayState.Stalled

    /** Where the player is in its song: the engine's own position when it holds that song. */
    private fun livePosition(p: Projection): Long {
        val engine = playback.engine
        val current = p.state.current
        return if (engine != null && current != null && engine.currentId == current.id) engine.positionMs else p.state.positionMs
    }

    /** Brings the player to the room if it has strayed, and tells the room how the player is doing. Run when the room changes and once a second. */
    private fun keepUp() {
        val now = _state.value
        if (!now.inRoom) return
        if (seed != Seed.None) return seedStep()
        val room = now.room ?: return
        val serverNow = serverNow()
        val p = playback.projection.value
        val clock = monotonicMs()
        val local = Local(
            following = p.followingRoom,
            queue = p.state.queue.items.map { it.id },
            current = p.state.current?.id,
            positionMs = livePosition(p),
            playing = wantsToPlay(p),
            settled = !wantsToPlay(p) || p.state.state == PlayState.Playing,
        )
        val since = correctedAt?.let { clock - it }
        val connection = line ?: return
        // The room's song is the one in the player and it will not play:
        // asking for it again each second would not make it, so the room is
        // told instead, and "Resync me" is how it is asked for again.
        val currentEntry = room.currentEntry()?.value
        val theRooms = currentEntry != null && appliedEntry == currentEntry.id && local.current == currentEntry.track.id
        if (theRooms && p.state.current?.playable == false) {
            _state.update { it.copy(standing = Standing.Unavailable) }
            connection.status(Standing.Unavailable.wire(room.playing), room.current)
            return
        }
        // The length as it was measured here, where the room's is missing or
        // a little out: the room ends a song by its length. Said once for
        // each song, by someone the room lets steer it.
        val measured = p.room?.takeIf { it.durationMs > 0 }
        if (measured != null && currentEntry != null && measured.entry == currentEntry.id &&
            local.current == currentEntry.track.id && room.mayControl(now.me) &&
            lengthWorthTelling(currentEntry.track.durationMs.coerceAtLeast(0.0).toLong(), measured.durationMs)
        ) {
            val told = "${measured.entry}:${currentEntry.track.id}"
            if (lengthTold != told) {
                lengthTold = told
                connection.command("duration", buildJsonObject {
                    put("entry", measured.entry); put("durationMs", measured.durationMs)
                })
            }
        }
        // The room moves on when a player says the song is over; it is said
        // again until it does, in case the first was lost.
        val endedHere = p.room?.ended == true && p.room.entry == room.current
        val standing = Standing.of(room, serverNow, local, endedHere)
        follow(room, serverNow, local, appliedEntry, since)?.let { command ->
            appliedEntry = room.current
            correctedAt = clock
            playback.send(command)
        }
        val due = endedSent?.let { clock - it >= ENDED_EVERY_MS } ?: true
        if (endedHere && room.playing && due) {
            endedSent = clock
            connection.command("ended", buildJsonObject { put("current", room.current) })
        }
        _state.update { it.copy(standing = standing) }
        connection.status(standing.wire(room.playing), room.current)
    }

    /** The next step of bringing the music that was playing into a new room. */
    private fun seedStep() {
        val room = _state.value.room ?: return
        val connection = line ?: return
        when (val step = seed) {
            Seed.None -> {}
            is Seed.Wanted -> {
                seed = Seed.Enqueued(step.positionMs, step.playing)
                connection.command("enqueue", buildJsonObject { put("tracks", tracksJson(step.tracks)) })
            }
            is Seed.Enqueued -> {
                // Not there yet: wait for the room to say it has the songs.
                val first = room.queue.firstOrNull() ?: return
                seed = Seed.Sought(step.playing)
                if (room.current == null) connection.command("jump", buildJsonObject { put("entry", first.id) })
                else if (step.positionMs > SEED_SEEK_FROM_MS) connection.command("seek", buildJsonObject { put("positionMs", step.positionMs) })
            }
            is Seed.Sought -> {
                seed = Seed.None
                if (step.playing && !room.playing) connection.command("play")
                else if (!step.playing && room.playing) connection.command("pause")
            }
        }
    }

    // ---------- the room's page ----------

    /** The room this listener is seated in with the line up, which is what a command needs; otherwise the reason to wait is said. */
    private fun seated(): Room? {
        if (_state.value.phase != Phase.Joined) {
            say("Wait for the room to reconnect.")
            return null
        }
        return _state.value.room
    }

    /** One command for the room, if this listener is seated in it. */
    private fun command(kind: String, fields: JsonObject = JsonObject(emptyMap())) {
        if (seated() != null) line?.command(kind, fields)
    }

    /** As [command], for what only the leader may ask. */
    private fun leaderCommand(kind: String, fields: JsonObject = JsonObject(emptyMap())) {
        val room = seated() ?: return
        if (!room.leads(_state.value.me)) return say(LEADER_SETS)
        line?.command(kind, fields)
    }

    /** Carries out what routing a command came to. */
    private fun routed(routed: Routed) {
        when (routed) {
            is Routed.ToRoom -> line?.command(routed.kind, routed.fields)
            is Routed.Refused -> say(routed.why)
            Routed.Core -> {}
        }
    }

    fun transport(transport: Transport) {
        val room = seated() ?: return
        if (!room.mayControl(_state.value.me)) return say(LEADER_PLAYS)
        when (transport) {
            Transport.Toggle -> line?.command(if (room.playing) "pause" else "play")
            Transport.Next -> line?.command("next")
            // "Previous" means back to the start or back a song by where the
            // song was when it was pressed; saying which keeps two presses at
            // once from doing both.
            Transport.Previous -> line?.command("previous", buildJsonObject {
                put("restart", room.positionAt(serverNow()) > RESTART_AFTER_MS)
            })
        }
    }

    /** Sends a press of the player's own controls to the room, as [route] decides. */
    fun press(command: Command) {
        val room = seated() ?: return
        if (trimmed(command)) say(FIRST_HUNDRED)
        routed(route(command, room, _state.value.me, room.positionAt(serverNow())))
    }

    fun set(setting: Setting) = leaderCommand("settings", setting.fields())

    /**
     * "Songs per guest" was left: sent only if it is a real change, so
     * leaving the field neither posts "updated room settings" nor makes
     * others' commands stale.
     */
    fun commitLimit(text: String) {
        val limit = text.filter(Char::isDigit).take(3).toIntOrNull() ?: return
        if (limit in 1..100 && limit != _state.value.room?.limit) leaderCommand("settings", buildJsonObject { put("limit", limit) })
    }

    fun rotatePin() = leaderCommand("rotate")

    /** Pauses everyone and asks whether they are ready. */
    fun readyCheck() = leaderCommand("countdown")

    /** Starts in three seconds, ready or not. */
    fun startSoon() = leaderCommand("countdown", buildJsonObject { put("force", true) })

    fun ready() = command("ready", buildJsonObject { put("ready", true) })

    fun voteSkip() = command("vote")

    fun endRoom() = leaderCommand("end")

    /** Makes a listener a DJ, or a DJ a listener again. */
    fun role(member: String, dj: Boolean) =
        leaderCommand("role", buildJsonObject { put("member", member); put("role", if (dj) "dj" else "listener") })

    fun makeLeader(member: String) = leaderCommand("transfer", buildJsonObject { put("member", member) })

    fun removeListener(member: String) = leaderCommand("kick", buildJsonObject { put("member", member) })

    /** Lets someone who is waiting in. */
    fun admit(request: String) = leaderCommand("approve", buildJsonObject { put("request", request) })

    fun turnAway(request: String) = leaderCommand("deny", buildJsonObject { put("request", request) })

    /** Puts requested songs in the queue: next, or at its end. */
    fun accept(requests: List<String>, next: Boolean) = command("acceptRequest", buildJsonObject {
        put("requests", buildJsonArray { requests.forEach { add(JsonPrimitive(it)) } })
        put("placement", if (next) "next" else "end")
    })

    fun decline(requests: List<String>) = command("declineRequest", buildJsonObject {
        put("requests", buildJsonArray { requests.forEach { add(JsonPrimitive(it)) } })
    })

    /** Withdraws a request of one's own; its going is then this listener's doing, not the leader's no. */
    fun withdraw(request: String) {
        if (seated() == null) return
        line?.command("cancelRequest", buildJsonObject { put("request", request) })
        withdrawn += request
    }

    /** Adds a song to the room, or asks for it where the leader approves. */
    fun add(track: Track) {
        val room = seated() ?: return
        routed(adding(room, _state.value.me, listOf(track), null))
    }

    /** Plays this entry of the queue now. */
    fun jump(entry: String) {
        val room = seated() ?: return
        if (!room.mayControl(_state.value.me)) return say(LEADER_PLAYS)
        line?.command("jump", buildJsonObject { put("entry", entry) })
    }

    fun remove(entry: String) = command("remove", buildJsonObject { put("entry", entry) })

    /** Takes back the last edit of the queue. */
    fun undo() = command("undo")

    /** Brings this player back to the room, now, as if it had never been brought to it. */
    fun resync() {
        appliedEntry = null
        correctedAt = null
        keepUp()
    }
}

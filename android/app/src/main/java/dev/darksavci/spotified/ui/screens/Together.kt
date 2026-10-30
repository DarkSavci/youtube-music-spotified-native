package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.ui.components.SearchField
import androidx.annotation.DrawableRes
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Account
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.SearchFilter
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.together.Entry
import dev.darksavci.spotified.together.Invite
import dev.darksavci.spotified.together.LEADER_PLAYS
import dev.darksavci.spotified.together.Member
import dev.darksavci.spotified.together.Mode
import dev.darksavci.spotified.together.Phase
import dev.darksavci.spotified.together.Policy
import dev.darksavci.spotified.together.Room
import dev.darksavci.spotified.together.ServerForm
import dev.darksavci.spotified.together.Setting
import dev.darksavci.spotified.together.SongRequest
import dev.darksavci.spotified.together.Together
import dev.darksavci.spotified.together.TogetherState
import dev.darksavci.spotified.together.Transport
import dev.darksavci.spotified.together.answers
import dev.darksavci.spotified.together.inviteQuestion
import dev.darksavci.spotified.together.inviteText
import dev.darksavci.spotified.together.leads
import dev.darksavci.spotified.together.mayAdd
import dev.darksavci.spotified.together.mayRemove
import dev.darksavci.spotified.together.mayUndo
import dev.darksavci.spotified.together.pictureOf
import dev.darksavci.spotified.together.pictureOfAdder
import dev.darksavci.spotified.together.ready
import dev.darksavci.spotified.together.requesting
import dev.darksavci.spotified.together.requestsFor
import dev.darksavci.spotified.together.savedIn
import dev.darksavci.spotified.together.spacedPin
import dev.darksavci.spotified.together.undoSecondsLeft
import dev.darksavci.spotified.together.upcoming
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.Change
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.ConfirmDialog
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Loading
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.Problem
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.artColor
import dev.darksavci.spotified.ui.components.darkened
import dev.darksavci.spotified.ui.components.rememberHaptics
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.coroutines.cancellation.CancellationException
import kotlin.math.ceil
import kotlinx.coroutines.delay

/*
 * The Listen Together page: choose a server, make or join a room, and while
 * in one, see what it plays, what waits, and who is there. Laid out as the
 * desktop's crates/app/src/views/together, one column for a phone.
 */

@Composable
fun TogetherScreen() {
    val actions = LocalActions.current
    val graph = actions.graph
    val together = graph.together
    val state by together.state.collectAsStateWithLifecycle()
    // The fields start from what was saved, so nothing is shown until it is read.
    val loaded by graph.settings.settings.collectAsStateWithLifecycle(null)
    val asked by produceState<Result<Account?>?>(null) { value = runCatching { graph.api.account() } }
    val settings = loaded ?: return Loading()
    val room = state.room
    if (state.inRoom && room != null) {
        RoomPage(together, state, room, settings)
    } else {
        Lobby(together, state, settings, asked?.getOrNull(), asked?.isFailure == true)
    }
    state.invite?.let { invite -> InviteDialog(together, state, settings, invite) }
}

/** A link was followed to a room: joining tells its server who this is, so it asks, and says which server. */
@Composable
private fun InviteDialog(together: Together, state: TogetherState, settings: Settings, invite: Invite) {
    val leaving = state.room?.takeIf { state.inRoom }?.title()
    ConfirmDialog(
        title = "Join this room?",
        text = inviteQuestion(invite, invite.savedIn(settings.togetherServers)?.name, leaving),
        action = "Join room",
        onConfirm = together::acceptInvite,
        onDismiss = together::dismissInvite,
        danger = false,
    )
}

// ---------- before a room ----------

@Composable
private fun Lobby(together: Together, state: TogetherState, settings: Settings, account: Account?, accountFailed: Boolean) {
    val actions = LocalActions.current
    val p = LocalPalette.current
    var removing by remember { mutableStateOf<ServerForm?>(null) }
    var connection by rememberSaveable { mutableStateOf(false) }
    LazyColumn(Modifier.windowInsetsPadding(WindowInsets.statusBars), contentPadding = PaddingValues(bottom = 32.dp)) {
        item {
            Row(verticalAlignment = Alignment.CenterVertically) {
                GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
                Text("Listen Together", style = Type.title, modifier = Modifier.semantics { heading() })
            }
            Quiet(
                "Play the same music at the same moment as your friends, each on your own account and at your own volume.",
                Modifier.padding(horizontal = 16.dp),
            )
        }
        state.error?.let { error -> item { Alert(error, together::dismissError) } }
        when (state.phase) {
            Phase.Waiting -> item {
                Panel {
                    PanelHeading("Waiting for the leader…")
                    Quiet("Your name and picture were sent for approval. You’ll join when they accept.")
                    Spacer(Modifier.height(12.dp))
                    Pill("Cancel request", { together.leave() })
                }
            }
            Phase.Connecting -> item {
                TextAction("Cancel connection", { together.leave() }, Modifier.padding(horizontal = 4.dp))
            }
            else -> {}
        }
        item { StartPanel(together, state, settings) }
        item { JoinPanel(together, state, settings) }
        // Nothing here is needed to start or join: asking for a room with no server opens the form.
        item {
            Panel {
                val server = settings.togetherServer?.name
                val name = settings.togetherName.trim().ifEmpty { account?.name?.ifEmpty { null } ?: "Listener" }
                Disclosure(
                    "Server and your name",
                    if (server != null) "$server · Joining as $name" else "No server yet · Joining as $name",
                    connection,
                ) { connection = !connection }
                if (connection) {
                    Spacer(Modifier.height(12.dp))
                    ServerBar(together, state, settings)
                    Spacer(Modifier.height(16.dp))
                    Identity(together, settings, account, accountFailed)
                    Spacer(Modifier.height(8.dp))
                    Small("Only your name, your picture if you share it, and what you do in a room are shared with the people in it.")
                }
            }
        }
    }
    state.manage?.let { form -> ServerSheet(together, form, onRemove = { removing = form }) }
    removing?.let { form ->
        val name = settings.togetherServers.firstOrNull { it.id == form.editing }?.name?.ifEmpty { null } ?: "this server"
        ConfirmDialog(
            title = "Remove $name?",
            text = "You can add it again later with its address.",
            action = "Remove",
            onConfirm = { removing = null; form.editing?.let(together::removeServer) },
            onDismiss = { removing = null },
        )
    }
}

@Composable
private fun StartPanel(together: Together, state: TogetherState, settings: Settings) {
    var roomName by rememberSaveable { mutableStateOf(settings.togetherRoomName) }
    var options by rememberSaveable { mutableStateOf(false) }
    Panel {
        PanelHeading("Start a room")
        Quiet("Make a room and invite your friends. You lead it.")
        Spacer(Modifier.height(12.dp))
        val connecting = state.phase == Phase.Connecting
        Button(
            if (connecting) "Connecting…" else "Create room",
            enabled = state.phase == Phase.Idle,
            icon = R.drawable.ic_plus,
            onClick = together::create,
        )
        Spacer(Modifier.height(12.dp))
        val named = settings.togetherRoomName.trim()
        Disclosure(
            "Room options",
            if (named.isNotEmpty()) "$named · ${settings.togetherMode.title}" else settings.togetherMode.title,
            options,
        ) { options = !options }
        if (options) {
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                roomName,
                { roomName = it.take(80); together.typeRoomName(it) },
                Modifier.fillMaxWidth(),
                label = { Text("Room name (optional)") },
                placeholder = { Text("Friday night together") },
                singleLine = true,
            )
            Small("Remembered on this device for your next room. Visible to listeners.")
            Spacer(Modifier.height(12.dp))
            Text("Who controls the music", style = Type.label)
            Spacer(Modifier.height(6.dp))
            Mode.entries.forEach { mode ->
                ModeChoice(mode, settings.togetherMode == mode) { together.chooseMode(mode) }
            }
        }
    }
}

/** A heading that opens what is under it, saying what is chosen there while it is shut. */
@Composable
private fun Disclosure(title: String, summary: String, open: Boolean, onToggle: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp))
            .tapRow(onClick = onToggle, role = Role.Button)
            .semantics { stateDescription = if (open) "Open" else "Closed" }
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = Type.bodyStrong)
            if (!open) Text(summary, style = Type.small, color = p.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Glyph(if (open) R.drawable.ic_chevron_up else R.drawable.ic_chevron_down, null, tint = p.secondary, size = 20.dp)
    }
}

/** One way a room can be run, as a row to choose: its name, what it means, and a mark on the one chosen. */
@Composable
private fun ModeChoice(mode: Mode, chosen: Boolean, onChoose: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp))
            .background(if (chosen) p.surfaceActive else p.panel)
            .tapRow(onClick = onChoose, role = Role.RadioButton)
            .semantics { selected = chosen }
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier.size(18.dp).clip(CircleShape)
                .then(if (chosen) Modifier.background(p.accent) else Modifier.border(1.5.dp, p.dim, CircleShape)),
            contentAlignment = Alignment.Center,
        ) { if (chosen) Box(Modifier.size(6.dp).clip(CircleShape).background(p.onAccent)) }
        Spacer(Modifier.width(12.dp))
        Column {
            Text(mode.title, style = Type.bodyStrong)
            Text(mode.about, style = Type.small, color = p.secondary)
        }
    }
}

@Composable
private fun JoinPanel(together: Together, state: TogetherState, settings: Settings) {
    val p = LocalPalette.current
    val idle = state.phase == Phase.Idle
    val whole = state.pin.length == 8
    Panel {
        PanelHeading("Join a room")
        Quiet("Enter the 8-digit PIN a friend gave you, or paste their link.")
        Spacer(Modifier.height(12.dp))
        OutlinedTextField(
            state.pin,
            together::typePin,
            Modifier.fillMaxWidth(),
            label = { Text("Room PIN or link") },
            placeholder = { Text("0000 0000", style = PinStyle, color = p.dim) },
            textStyle = PinStyle,
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Go),
            keyboardActions = KeyboardActions(onGo = { if (idle && whole) together.join() }),
        )
        Spacer(Modifier.height(12.dp))
        Button("Join room", enabled = idle && whole, onClick = together::join)
        Spacer(Modifier.height(12.dp))
        // A link brings its own server; a PIN only works on the one chosen here.
        settings.togetherServer?.let { Small("A PIN works on the server your friend uses. Using ${it.name}.") }
        Small("Your own queue waits while you are in a room. When you leave, the song playing carries on, then your own queue.")
    }
}

/** The figures stand apart, as they are read: one at a time. */
private val PinStyle = TextStyle(fontSize = 24.sp, letterSpacing = 6.sp)

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ServerBar(together: Together, state: TogetherState, settings: Settings) {
    Titled("Server", "Rooms are made and joined here. Your friends need the same server.")
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        settings.togetherServers.forEach { server ->
            Chip(server.name, server.id == settings.togetherSelected, { together.selectServer(server.id) })
        }
        if (settings.togetherServers.isEmpty()) Small("No saved servers yet.")
    }
    Spacer(Modifier.height(8.dp))
    val label = if (settings.togetherServer != null) "Manage" else "Add server"
    if (state.manage == null) Pill(label, together::toggleManage, icon = if (settings.togetherServer != null) R.drawable.ic_pencil else R.drawable.ic_plus)
}

/** The form that adds a server or edits one, over the page, as starting or joining with no server opens it. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ServerSheet(together: Together, form: ServerForm, onRemove: () -> Unit) {
    val p = LocalPalette.current
    val editing = form.editing != null
    ModalBottomSheet(
        onDismissRequest = together::closeManage,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = p.panel,
        contentColor = p.text,
    ) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).imePadding().padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
            Text(if (editing) "Edit server" else "Add a server", style = Type.heading, modifier = Modifier.semantics { heading() })
            Small("Rooms run on a server you and your friends share. Ask whoever set it up for its address. You only need to save it once.")
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                form.name, together::serverName, Modifier.fillMaxWidth(),
                label = { Text("Name") }, placeholder = { Text("Our music room") }, singleLine = true,
            )
            OutlinedTextField(
                form.url, together::serverAddress, Modifier.fillMaxWidth(),
                label = { Text("Address") }, placeholder = { Text("wss://listen.example.com") }, singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            )
            if (form.check.isNotEmpty()) {
                Spacer(Modifier.height(6.dp))
                Text(
                    form.check, style = Type.small, color = p.secondary,
                    modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
                )
            }
            Spacer(Modifier.height(10.dp))
            Buttons {
                Button("Save server", onClick = together::saveServer)
                Pill("Test connection", together::testServer)
                Pill("Add another", together::addAnother)
                if (editing) TextAction("Remove saved server", onRemove, color = p.danger)
                TextAction("Cancel", together::closeManage)
            }
        }
    }
}

/** The picture and name this listener is seen by. */
@Composable
private fun Identity(together: Together, settings: Settings, account: Account?, accountFailed: Boolean) {
    val accountName = account?.name?.ifEmpty { null } ?: "Listener"
    var name by rememberSaveable { mutableStateOf(settings.togetherName) }
    val shown = name.trim().ifEmpty { accountName }
    val hasPicture = !account?.avatarUrl.isNullOrEmpty()
    val shares = hasPicture && settings.togetherSharePicture
    Titled("Your name", "What the people in a room see you as.")
    Row(verticalAlignment = Alignment.CenterVertically) {
        OutlinedTextField(
            name, { name = it.take(50); together.typeName(it) }, Modifier.weight(1f),
            label = { Text("Joining as") }, placeholder = { Text(accountName) }, singleLine = true,
        )
        Spacer(Modifier.width(12.dp))
        Avatar(shown, if (shares) pictureOf(account?.avatarUrl.orEmpty()) else emptyList(), 40)
    }
    if (accountFailed) Small("Your account could not be read, so your name and picture from it aren’t shown.")
    if (hasPicture) {
        Spacer(Modifier.height(6.dp))
        Toggle("Share my profile picture", "Shown beside your name. Otherwise your initial is.", shares, together::sharePicture)
    }
}

// ---------- in a room ----------

private enum class Tab { Queue, History, Activity }

@Composable
private fun RoomPage(together: Together, state: TogetherState, room: Room, settings: Settings) {
    val actions = LocalActions.current
    val me = state.me
    val leads = room.leads(me)
    var tab by rememberSaveable { mutableStateOf(Tab.Queue) }
    var settingsOpen by rememberSaveable { mutableStateOf(false) }
    var leaving by rememberSaveable { mutableStateOf<String?>(null) }
    var removing by remember { mutableStateOf<Member?>(null) }
    var savingHistory by remember { mutableStateOf<String?>(null) }
    // The room says whether an edit can be taken back, so the Undo comes with its answer.
    LaunchedEffect(together) {
        together.undoOffers.collect { said -> actions.sayWithUndo(said) { together.undo() } }
    }
    LazyColumn(contentPadding = PaddingValues(bottom = 32.dp)) {
        item { Head(state, room) }
        item {
            RoomButtons(
                together, state, room, me, settings,
                onSettings = { settingsOpen = !settingsOpen },
                onLeave = { leaving = "" },
            )
        }
        state.error?.let { error -> item { Alert(error, together::dismissError) } }
        if (settingsOpen && leads) item { RoomSettings(together, room) }
        if (room.countdown != null) item { ReadyCheck(together, room, me) }
        item {
            Row(
                Modifier.padding(start = 16.dp, end = 16.dp, top = 20.dp, bottom = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Chip("Queue · ${room.upcoming().size}", tab == Tab.Queue, { tab = Tab.Queue })
                Chip("History", tab == Tab.History, { tab = Tab.History })
                Chip("Activity", tab == Tab.Activity, { tab = Tab.Activity })
            }
        }
        when (tab) {
            Tab.Queue -> queue(together, room, me)
            Tab.History -> history(room) { savingHistory = "Listen Together" }
            Tab.Activity -> activity(room)
        }
        item { Listeners(together, room, me, settings, onRemove = { removing = it }) }
    }
    leaving?.let { next ->
        LeaveDialog(
            room, me, next,
            onNext = { leaving = it },
            onLeave = { leaving = null; together.leave(next) },
            onStay = { leaving = null },
            onEnd = { leaving = null; together.endRoom() },
        )
    }
    removing?.let { member ->
        ConfirmDialog(
            title = "Remove ${member.name}?",
            text = "They leave the room and the PIN changes, so share the new PIN with everyone who is still joining.",
            action = "Remove",
            onConfirm = { removing = null; together.removeListener(member.id) },
            onDismiss = { removing = null },
        )
    }
    savingHistory?.let { name ->
        SaveHistory(actions, name, room, onName = { savingHistory = it }, onDone = { savingHistory = null })
    }
}

/** The song playing, large, over its cover's colour, under the room's name: what an album's page says of the album, the room says of itself. */
@Composable
private fun Head(state: TogetherState, room: Room) {
    val actions = LocalActions.current
    val p = LocalPalette.current
    val current = room.currentEntry()?.value
    val art = current?.track?.artwork.orEmpty()
    val tint = artColor(art).darkened()
    val listening = room.members.count { it.connected }
    Column(
        Modifier.fillMaxWidth().background(Brush.verticalGradient(listOf(tint, p.window)))
            .windowInsetsPadding(WindowInsets.statusBars),
    ) {
        GlyphButton(R.drawable.ic_chevron_left, "Back", { actions.nav.popBackStack() })
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            if (current != null) Art(art, 220.dp, description = "Cover of ${current.track.title}")
            else Box(Modifier.size(220.dp).clip(RoundedCornerShape(4.dp)).background(p.surface), contentAlignment = Alignment.Center) {
                Glyph(R.drawable.ic_headphones, null, tint = p.dim, size = 64.dp)
            }
        }
        Spacer(Modifier.height(16.dp))
        Column(Modifier.padding(horizontal = 16.dp)) {
            Text("LISTEN TOGETHER", style = Type.tiny, color = p.secondary)
            Text(room.title(), style = Type.display, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Spacer(Modifier.height(4.dp))
            val playing = when {
                current != null -> "Playing ${current.track.title} · ${current.track.artistNames}"
                !room.mayAdd(state.me) -> "The leader picks the first song"
                else -> "Nothing playing yet"
            }
            Text(playing, style = Type.bodyStrong, maxLines = 2, overflow = TextOverflow.Ellipsis)
            val standing = if (state.phase == Phase.Reconnecting) "Reconnecting…" else state.standing.label
            val listeners = if (listening == 1) "1 listener" else "$listening listeners"
            Text("$listeners · ${room.mode.title} · $standing", style = Type.small, color = p.secondary)
        }
    }
}

/**
 * Under the room's head: play or pause, the songs either side and the vote
 * to skip, for those who steer it; then the PIN that invites, the way out,
 * and for the leader the room's settings.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RoomButtons(together: Together, state: TogetherState, room: Room, me: String, settings: Settings, onSettings: () -> Unit, onLeave: () -> Unit) {
    val actions = LocalActions.current
    val haptics = rememberHaptics()
    // A command needs the line up as well as the right to send it; until then the buttons wait with the room.
    val seated = state.phase == Phase.Joined
    val steers = room.mayControl(me) && seated
    Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        RoomPlayButton(room.playing, enabled = steers && room.current != null) {
            haptics.toggle(!room.playing)
            together.transport(Transport.Toggle)
        }
        Spacer(Modifier.width(8.dp))
        RoomGlyph(R.drawable.ic_skip_back_filled, "Previous song", steers) { together.transport(Transport.Previous) }
        RoomGlyph(R.drawable.ic_skip_forward_filled, "Next song", steers) { together.transport(Transport.Next) }
        Spacer(Modifier.weight(1f))
        if (room.leads(me)) GlyphButton(R.drawable.ic_settings, "Room settings", onSettings)
    }
    if (!room.mayControl(me)) Small(LEADER_PLAYS, Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp))
    FlowRow(
        Modifier.padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Pill(
            "PIN  ${spacedPin(room.pin)}",
            { actions.copy(room.pin, "PIN copied. Friends need the same server.") },
            icon = R.drawable.ic_copy,
            description = "Copy PIN ${spacedPin(room.pin)}",
        )
        Pill("Invite", {
            val server = settings.togetherServer?.url.orEmpty()
            actions.share("Listen Together", inviteText(room.title(), server, room.pin))
        }, icon = R.drawable.ic_share_2, description = "Invite friends")
        if (room.voteSkip) Pill("Vote to skip · ${room.votes.size}", together::voteSkip, enabled = seated)
        Pill("Leave room", onLeave, icon = R.drawable.ic_log_out)
    }
    room.lastControlledBy?.let { person ->
        Small("Last controlled by ${person.name}", Modifier.padding(horizontal = 16.dp, vertical = 6.dp))
    }
}

/** The room's play button, which can be off: without the right to steer it does nothing, and says so to TalkBack. */
@Composable
private fun RoomPlayButton(playing: Boolean, enabled: Boolean, onClick: () -> Unit) {
    val p = LocalPalette.current
    val label = if (playing) "Pause" else "Play"
    Box(
        Modifier.size(56.dp).clip(CircleShape).background(if (enabled) p.accent else p.surfaceActive)
            .clickable(enabled = enabled, onClickLabel = label, role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Glyph(if (playing) R.drawable.ic_pause_filled else R.drawable.ic_play_filled, label, tint = if (enabled) p.onAccent else p.dim, size = 24.dp)
    }
}

/** An icon button of the room's, which can be off in the same way. */
@Composable
private fun RoomGlyph(@DrawableRes icon: Int, description: String, enabled: Boolean, onClick: () -> Unit) {
    val p = LocalPalette.current
    Box(
        Modifier.size(48.dp).clip(CircleShape)
            .clickable(enabled = enabled, onClickLabel = description, role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Glyph(icon, description, tint = if (enabled) p.text else p.dim) }
}

/** How to leave: for the leader, who leads next, or the end of the room. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LeaveDialog(room: Room, me: String, next: String, onNext: (String) -> Unit, onLeave: () -> Unit, onStay: () -> Unit, onEnd: () -> Unit) {
    val p = LocalPalette.current
    var ending by rememberSaveable { mutableStateOf(false) }
    if (ending) {
        ConfirmDialog(
            title = "End the room for everyone?",
            text = "Everyone leaves and the room’s queue is gone. This can’t be undone.",
            action = "End room",
            onConfirm = onEnd,
            onDismiss = { ending = false },
        )
        return
    }
    if (!room.leads(me)) {
        ConfirmDialog(
            title = "Leave this room?",
            text = "The song playing carries on, then your own queue.",
            action = "Leave room",
            onConfirm = onLeave,
            onDismiss = onStay,
            danger = false,
            dismiss = "Stay",
        )
        return
    }
    val others = room.members.filter { it.connected && it.id != me }
    AlertDialog(
        onDismissRequest = onStay,
        containerColor = p.overlay,
        titleContentColor = p.text,
        textContentColor = p.secondary,
        title = { Text("Keep the music going?", style = Type.heading) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                Text(
                    "Choose the next leader, or let us pick a connected listener. The room and queue stay together, " +
                        "and the music keeps playing here from the room’s queue.",
                    style = Type.body,
                )
                Spacer(Modifier.height(12.dp))
                if (others.isEmpty()) {
                    Text("No one else is connected right now.", style = Type.small)
                } else {
                    Text("Next leader", style = Type.label, color = p.text)
                    Spacer(Modifier.height(6.dp))
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Chip("Choose someone at random", others.none { it.id == next }, { onNext("") })
                        others.forEach { member -> Chip(member.name, member.id == next, { onNext(member.id) }) }
                    }
                }
                Spacer(Modifier.height(8.dp))
                TextAction("End room for everyone", { ending = true }, color = p.danger)
            }
        },
        confirmButton = { TextButton(onLeave) { Text("Leave room", style = Type.bodyStrong, color = p.accent) } },
        dismissButton = { TextButton(onStay) { Text("Stay", style = Type.bodyStrong, color = p.text) } },
    )
}

/** The leader's controls over who steers, who joins, and the queue. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RoomSettings(together: Together, room: Room) {
    Panel {
        PanelHeading("Room settings")
        Text("Who controls playback?", style = Type.label)
        Spacer(Modifier.height(6.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Mode.entries.forEach { mode -> Chip(mode.title, mode == room.mode, { together.set(Setting.ModeTo(mode)) }) }
        }
        Small(room.mode.about)
        Spacer(Modifier.height(12.dp))
        Text("Queue order", style = Type.label)
        Spacer(Modifier.height(6.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Policy.entries.forEach { policy -> Chip(policy.title, policy == room.policy, { together.set(Setting.PolicyTo(policy)) }) }
        }
        Small(
            when (room.policy) {
                Policy.Fifo -> "Songs play in the order they were added."
                Policy.Turns -> "One song from each person in turn, so no one takes over the queue."
            },
        )
        Spacer(Modifier.height(12.dp))
        LimitField(together, room)
        Small("How many songs each guest can have waiting in the queue, from 1 to 100.")
        Spacer(Modifier.height(6.dp))
        Toggle("Approve new listeners", "You let each person in before they can join, even with the PIN.", room.joinApproval) { together.set(Setting.JoinApproval(it)) }
        Toggle("Lock new joins", "No one new can join, even with the PIN. Everyone here stays.", room.locked) { together.set(Setting.Locked(it)) }
        if (room.mode == Mode.Contributions) {
            Toggle("Add requests without asking me", "Friends’ requests go straight into the queue.", room.autoAccept) { together.set(Setting.AutoAccept(it)) }
        }
        Toggle("Allow duplicates", "The same song can wait in the queue more than once.", room.duplicates) { together.set(Setting.Duplicates(it)) }
        Toggle("Vote to skip", "Listeners can vote to skip the song playing.", room.voteSkip) { together.set(Setting.VoteSkip(it)) }
        Spacer(Modifier.height(8.dp))
        Buttons {
            Pill("Rotate PIN", together::rotatePin)
            Pill("Start together / ready check", together::readyCheck)
        }
        Small("Rotating the PIN: everyone stays. The old PIN and any invite links stop working.")
        Small("A ready check pauses everyone and starts once they have all said they are ready.")
    }
}

/** "Songs per guest": typed, and sent when the field is left. */
@Composable
private fun LimitField(together: Together, room: Room) {
    var text by remember(room.limit) { mutableStateOf(room.limit.toString()) }
    var focused by remember { mutableStateOf(false) }
    OutlinedTextField(
        text,
        { text = it.filter(Char::isDigit).take(3) },
        Modifier.fillMaxWidth().onFocusChanged {
            if (focused && !it.isFocused) together.commitLimit(text)
            focused = it.isFocused
        },
        label = { Text("Songs per guest") },
        placeholder = { Text("50") },
        singleLine = true,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { together.commitLimit(text) }),
    )
}

/** A shared start: who is ready, and the count once everyone is. */
@Composable
private fun ReadyCheck(together: Together, room: Room, me: String) {
    val countdown = room.countdown ?: return
    var now by remember { mutableDoubleStateOf(together.serverNow()) }
    val startAt = countdown.startAt
    // The count is the relay's clock running down.
    LaunchedEffect(startAt) {
        while (startAt != null) {
            now = together.serverNow()
            delay(250)
        }
    }
    val said = when {
        startAt == null -> "Ready for a shared start?"
        startAt > now -> "Starting in ${ceil((startAt - now) / 1000).toInt()}…"
        else -> "Starting…"
    }
    val (ready, connected) = room.ready()
    Panel {
        PanelHeading(said, Modifier.semantics { liveRegion = LiveRegionMode.Polite })
        Quiet("$ready of $connected ready")
        Spacer(Modifier.height(10.dp))
        Buttons {
            Button("I’m ready", onClick = together::ready)
            if (room.leads(me)) Pill("Start in 3 seconds", together::startSoon)
        }
    }
}

/** What waits: the songs asked for, the search that finds more, and the queue from the song playing on. */
private fun LazyListScope.queue(together: Together, room: Room, me: String) {
    // Always in the list while the room offers one: the row itself goes when the time is up.
    if (room.undo != null) item(key = "undo") { UndoEdit(together, room, me) }
    if (room.mayAdd(me)) item { RoomSearch(together, room, me) }
    val requests = room.requestsFor(me)
    if (requests.isNotEmpty()) {
        item {
            val answers = room.answers(me)
            val haptics = rememberHaptics()
            SectionTitle(
                if (answers) "Requests" else "Your requests",
                action = if (answers && requests.size > 1) "Accept all" else null,
                onAction = {
                    haptics.drop()
                    together.accept(requests.map { it.id }, next = false)
                },
            )
        }
        items(requests, key = { "request-" + it.id }) { request -> RequestRow(together, room, me, request) }
    }
    val upcoming = room.upcoming()
    if (upcoming.isEmpty()) {
        item { Quiet("Good music starts with one song.", Modifier.padding(16.dp)) }
    }
    items(upcoming, key = { "entry-" + it.value.id }) { (_, entry) ->
        val playing = entry.id == room.current
        val haptics = rememberHaptics()
        EntryRow(
            entry, playing, room.pictureOfAdder(entry),
            onClick = if (room.mayControl(me) && !playing) ({ together.jump(entry.id) }) else null,
            onRemove = if (room.mayRemove(me, entry)) ({ haptics.drop(); together.remove(entry.id) }) else null,
        )
    }
}

/** The offer to take back the last edit, with how long it lasts, gone when the relay would refuse it. */
@Composable
private fun UndoEdit(together: Together, room: Room, me: String) {
    var now by remember { mutableDoubleStateOf(together.serverNow()) }
    LaunchedEffect(room.revision, room.undo?.expires) {
        while (room.mayUndo(me, together.serverNow())) {
            now = together.serverNow()
            delay(250)
        }
        now = together.serverNow()
    }
    if (!room.mayUndo(me, now)) return
    TextAction("Undo edit · ${room.undoSecondsLeft(now)} s", together::undo, Modifier.padding(horizontal = 4.dp))
}

/** Looking for a song from inside the room, to add it or ask for it. */
@Composable
private fun RoomSearch(together: Together, room: Room, me: String) {
    val actions = LocalActions.current
    val p = LocalPalette.current
    var query by rememberSaveable { mutableStateOf("") }
    var results by remember { mutableStateOf<List<Track>?>(null) }
    var failed by remember { mutableStateOf<String?>(null) }
    var attempt by remember { mutableIntStateOf(0) }
    val requesting = room.requesting(me)
    LaunchedEffect(query, attempt) {
        results = null
        failed = null
        if (query.isBlank()) return@LaunchedEffect
        // Typing has paused: search.
        delay(400)
        runCatching {
            val found = actions.graph.api.search(query.trim(), SearchFilter.Songs)
            found.shelves.flatMap { it.items }.filterIsInstance<Item.Song>().map { it.track }.take(20)
        }.fold(
            { results = it },
            {
                if (it is CancellationException) throw it
                failed = it.message ?: "The search did not work."
            },
        )
    }
    Column(Modifier.padding(horizontal = 16.dp)) {
        SearchField(query, { query = it }, Modifier.fillMaxWidth(), placeholder = if (requesting) "Find songs to request" else "Find songs to add")
        if (query.isNotBlank()) {
            val found = results
            when {
                failed != null -> Problem(failed.orEmpty(), { attempt++ })
                found == null -> Quiet("Searching…", Modifier.padding(vertical = 12.dp))
                found.isEmpty() -> Quiet("No songs found. Try another search.", Modifier.padding(vertical = 12.dp))
                else -> found.forEach { track ->
                    Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Art(track.artwork, 44.dp)
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(track.title, style = Type.body, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(track.artistNames, style = Type.small, color = p.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        Pill(if (requesting) "Request" else "Add", { together.add(track) })
                    }
                }
            }
        }
    }
}

@Composable
private fun RequestRow(together: Together, room: Room, me: String, request: SongRequest) {
    val p = LocalPalette.current
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Art(request.track.artwork, 44.dp)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(request.track.title, style = Type.body, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Avatar(request.by.name, request.by.picture, 16)
                    Spacer(Modifier.width(4.dp))
                    Text("${request.track.artistNames} · from ${request.by.name}", style = Type.small, color = p.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
        Row(Modifier.padding(start = 56.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
            if (room.answers(me)) {
                TextAction("Decline", { together.decline(listOf(request.id)) }, color = p.secondary)
                TextAction("Add to queue", { together.accept(listOf(request.id), next = false) })
                TextAction("Play next", { together.accept(listOf(request.id), next = true) })
            } else {
                Text("Pending", style = Type.tiny, color = p.secondary, modifier = Modifier.padding(horizontal = 12.dp))
                TextAction("Cancel", { together.withdraw(request.id) }, color = p.secondary)
            }
        }
    }
}

@Composable
private fun EntryRow(entry: Entry, playing: Boolean, adder: List<Artwork>, onClick: (() -> Unit)?, onRemove: (() -> Unit)?) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth()
            // A swipe across the list must not jump the whole room to this song.
            .then(if (onClick != null) Modifier.tapRow(onClick = onClick, role = Role.Button, onClickLabel = "Play ${entry.track.title}") else Modifier)
            .padding(start = 16.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Art(entry.track.artwork, 48.dp)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(
                entry.track.title, style = Type.body, maxLines = 1, overflow = TextOverflow.Ellipsis,
                color = if (playing) p.accent else p.text,
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                // Radio is nobody's pick, so it has no face.
                if (!entry.radio) {
                    Avatar(entry.addedBy.name, adder, 16)
                    Spacer(Modifier.width(4.dp))
                }
                val by = if (entry.radio) "Radio" else "Added by ${entry.addedBy.name}"
                Text("${entry.track.artistNames} · $by", style = Type.small, color = p.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        if (onRemove != null) GlyphButton(R.drawable.ic_x, "Remove ${entry.track.title}", onRemove, tint = p.secondary, size = 18.dp)
    }
}

private fun LazyListScope.history(room: Room, onSave: () -> Unit) {
    if (room.history.isEmpty()) {
        item { Quiet("Songs played in this room will appear here.", Modifier.padding(16.dp)) }
        return
    }
    item { Pill("Save as playlist", onSave, Modifier.padding(horizontal = 16.dp, vertical = 4.dp), icon = R.drawable.ic_list_plus) }
    items(room.history.reversed(), key = { "history-" + it.id }) { entry -> EntryRow(entry, false, room.pictureOfAdder(entry), null, null) }
}

private fun LazyListScope.activity(room: Room) {
    if (room.activity.isEmpty()) {
        item { Quiet("Nothing has happened yet.", Modifier.padding(16.dp)) }
        return
    }
    items(room.activity.reversed(), key = { "activity-" + it.id }) { line ->
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp)) {
            Text(clockOf(line.at), style = Type.small, color = LocalPalette.current.secondary, modifier = Modifier.width(52.dp))
            Text(line.text, style = Type.body)
        }
    }
}

private val TimeOfDay = DateTimeFormatter.ofPattern("HH:mm")

/** A moment on the relay's clock, as this device's time of day. */
private fun clockOf(ms: Double): String = TimeOfDay.format(Instant.ofEpochMilli(ms.toLong()).atZone(ZoneId.systemDefault()))

/** Who is in the room, who waits to be let in, and this listener's own preferences. */
@Composable
private fun Listeners(together: Together, room: Room, me: String, settings: Settings, onRemove: (Member) -> Unit) {
    val p = LocalPalette.current
    val leads = room.leads(me)
    Panel {
        if (leads && room.pending.isNotEmpty()) {
            PanelHeading("Waiting to join")
            room.pending.forEach { knock ->
                Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Avatar(knock.name, knock.picture, 32)
                    Spacer(Modifier.width(10.dp))
                    Text(knock.name, style = Type.body, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    TextAction("Accept", { together.admit(knock.id) })
                    GlyphButton(R.drawable.ic_x, "Decline ${knock.name}", { together.turnAway(knock.id) }, tint = p.secondary, size = 18.dp)
                }
            }
            Spacer(Modifier.height(12.dp))
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            PanelHeading("In the room", Modifier.weight(1f))
            Text(room.members.size.toString(), style = Type.bodyStrong, color = p.secondary)
        }
        room.members.forEach { member -> ListenerRow(together, room, me, member, onRemove) }
        Spacer(Modifier.height(12.dp))
        Small("Room expires at ${clockOf(room.expires)}")
        Spacer(Modifier.height(12.dp))
        Text("Your experience", style = Type.label)
        Toggle("Show room activity notifications", null, settings.togetherNotifications, together::notifications)
        TextAction("Resync me", together::resyncMe, Modifier.padding(start = 0.dp))
        Small("Only affects your playback.")
    }
}

/** One listener: their picture, their name, what they are and are doing, and for the leader a menu of what can be done about them. */
@Composable
private fun ListenerRow(together: Together, room: Room, me: String, member: Member, onRemove: (Member) -> Unit) {
    val p = LocalPalette.current
    val manages = room.leads(me) && member.id != me
    var menu by remember { mutableStateOf(false) }
    val role = when {
        room.leads(member.id) -> "Leader"
        member.role == "dj" -> "DJ"
        else -> "Listener"
    }
    val doing = if (member.connected) member.status else "Reconnecting"
    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Avatar(member.name, member.picture, 36)
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f)) {
            Text(member.name + if (member.id == me) " (you)" else "", style = Type.bodyStrong, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("$role · $doing", style = Type.small, color = if (member.connected) p.secondary else p.danger, maxLines = 1)
        }
        if (manages) {
            Box {
                GlyphButton(R.drawable.ic_settings, "Manage ${member.name}", { menu = true }, tint = p.secondary, size = 18.dp)
                DropdownMenu(menu, { menu = false }) {
                    val dj = member.role == "dj"
                    DropdownMenuItem(
                        { Text(if (dj) "Make listener" else "Make DJ") },
                        { menu = false; together.role(member.id, !dj) },
                        leadingIcon = { Glyph(if (dj) R.drawable.ic_headphones else R.drawable.ic_disc_3, null, size = 18.dp) },
                    )
                    DropdownMenuItem(
                        { Text("Make leader") },
                        { menu = false; together.makeLeader(member.id) },
                        leadingIcon = { Glyph(R.drawable.ic_user, null, size = 18.dp) },
                        enabled = member.connected,
                    )
                    DropdownMenuItem(
                        { Text("Remove & rotate PIN", color = p.danger) },
                        { menu = false; onRemove(member) },
                    )
                }
            }
        }
    }
}

/** Makes a playlist of what the room has played, by the name given. */
@Composable
private fun SaveHistory(actions: Actions, name: String, room: Room, onName: (String) -> Unit, onDone: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDone,
        title = { Text("Save as playlist") },
        text = { OutlinedTextField(name, onName, label = { Text("Playlist name") }, singleLine = true) },
        confirmButton = {
            TextButton(
                {
                    val title = name.trim()
                    val ids = room.history.map { it.track.id }
                    onDone()
                    actions.write(Change.Library, done = "Saved “$title” to your library.") {
                        val id = actions.graph.api.createPlaylist(title)
                        actions.graph.api.addToPlaylist(id, ids)
                    }
                },
                enabled = name.isNotBlank(),
            ) { Text("Save") }
        },
        dismissButton = { TextButton(onDone) { Text("Cancel") } },
    )
}

// ---------- parts ----------

/** A card the page is made of. */
@Composable
private fun Panel(content: @Composable ColumnScope.() -> Unit) {
    Column(
        Modifier.padding(horizontal = 16.dp, vertical = 8.dp).fillMaxWidth()
            .clip(RoundedCornerShape(12.dp)).background(LocalPalette.current.surface).padding(16.dp),
        content = content,
    )
}

@Composable
private fun PanelHeading(text: String, modifier: Modifier = Modifier) {
    Text(text, style = Type.heading, modifier = modifier.semantics { heading() }.padding(bottom = 4.dp))
}

@Composable
private fun Quiet(text: String, modifier: Modifier = Modifier) {
    Text(text, style = Type.body, color = LocalPalette.current.secondary, modifier = modifier)
}

@Composable
private fun Small(text: String, modifier: Modifier = Modifier) {
    Text(text, style = Type.small, color = LocalPalette.current.secondary, modifier = modifier.padding(top = 4.dp))
}

@Composable
private fun Titled(title: String, subtitle: String) {
    Text(title, style = Type.bodyStrong)
    Text(subtitle, style = Type.small, color = LocalPalette.current.secondary, modifier = Modifier.padding(bottom = 8.dp))
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Buttons(content: @Composable () -> Unit) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        itemVerticalAlignment = Alignment.CenterVertically,
    ) { content() }
}

/** The strong button of a panel, in the accent colour. */
@Composable
private fun Button(text: String, enabled: Boolean = true, @DrawableRes icon: Int? = null, onClick: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.clip(RoundedCornerShape(20.dp)).background(if (enabled) p.accent else p.surfaceActive)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick).padding(horizontal = 18.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val ink = if (enabled) p.onAccent else p.dim
        if (icon != null) {
            Glyph(icon, null, tint = ink, size = 16.dp)
            Spacer(Modifier.width(6.dp))
        }
        Text(text, style = Type.bodyStrong, color = ink)
    }
}

@Composable
private fun TextAction(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, color: Color = LocalPalette.current.accent) {
    Text(
        text, style = Type.label, color = color,
        modifier = modifier.clip(RoundedCornerShape(12.dp)).clickable(role = Role.Button, onClick = onClick).padding(horizontal = 12.dp, vertical = 10.dp),
    )
}

@Composable
private fun Toggle(title: String, subtitle: String?, on: Boolean, onChange: (Boolean) -> Unit) {
    val p = LocalPalette.current
    val haptics = rememberHaptics()
    Row(
        // A swipe across the page must not change a room's setting; the switch only shows it.
        Modifier.fillMaxWidth()
            .tapRow(onClick = { haptics.toggle(!on); onChange(!on) }, role = Role.Switch)
            .semantics { toggleableState = ToggleableState(on) }
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = Type.body)
            if (subtitle != null) Text(subtitle, style = Type.small, color = p.secondary)
        }
        Spacer(Modifier.width(8.dp))
        Switch(on, null, colors = SwitchDefaults.colors(checkedTrackColor = p.accent))
    }
}

/** Something that went wrong, on a tint of the colour that says so, until it is dismissed. */
@Composable
private fun Alert(text: String, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.padding(horizontal = 16.dp, vertical = 8.dp).fillMaxWidth().clip(RoundedCornerShape(10.dp))
            .background(p.danger.copy(alpha = 0.12f)).border(1.dp, p.danger.copy(alpha = 0.3f), RoundedCornerShape(10.dp))
            .semantics { liveRegion = LiveRegionMode.Polite }
            .padding(start = 16.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text, style = Type.body, modifier = Modifier.weight(1f))
        GlyphButton(R.drawable.ic_x, "Dismiss error", onDismiss, size = 18.dp)
    }
}

/** A listener's picture, or their initial where they share none. */
@Composable
private fun Avatar(name: String, picture: List<Artwork>, size: Int) {
    val p = LocalPalette.current
    if (picture.isNotEmpty()) {
        Art(picture, size.dp, shape = CircleShape)
    } else {
        Box(Modifier.size(size.dp).clip(CircleShape).background(p.surfaceActive), contentAlignment = Alignment.Center) {
            // Not this phone's language: a Turkish phone must not make an "i" a dotted capital in someone's English name.
            Text(name.trim().take(1).uppercase(Locale.ROOT), style = if (size < 24) Type.tiny else Type.bodyStrong, textAlign = TextAlign.Center)
        }
    }
}

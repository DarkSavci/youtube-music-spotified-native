package dev.darksavci.spotified.ui.components

import androidx.annotation.DrawableRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

/** The bar of a page that has a plain title: back and the name. It stays through loading and failure. */
@Composable
fun TitleBar(title: String, modifier: Modifier = Modifier, actions: @Composable () -> Unit = {}) {
    val nav = LocalActions.current.nav
    // The back button sits where the fading bar has it, so that it does not move when a page arrives.
    Row(modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.statusBars).padding(horizontal = 4.dp, vertical = 2.dp), verticalAlignment = Alignment.CenterVertically) {
        GlyphButton(R.drawable.ic_chevron_left, "Back", { nav.popBackStack() })
        Text(title, style = Type.title, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        actions()
    }
}

/**
 * A page whose top bar is drawn by its content once it has loaded: until then,
 * through [skeleton] or a failure, it still has a way back.
 */
@Composable
fun <T> SubPage(
    loader: Loader<T>,
    title: String = "",
    skeleton: @Composable () -> Unit = { SkeletonCollection() },
    content: @Composable (T) -> Unit,
) {
    loader.state.collectAsStateWithLifecycle().value
    when (val s = loader.state.value) {
        is Load.Ready -> content(s.value)
        else -> Box(Modifier.fillMaxSize()) {
            if (s is Load.Failed) {
                Column(Modifier.fillMaxWidth().padding(top = 96.dp)) { Problem(s.message, { loader.reload() }) }
            } else {
                skeleton()
            }
            TitleBar(title)
        }
    }
}

/** Where an album's or a playlist's page will be: its art, its words and its rows. */
@Composable
fun SkeletonCollection(modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.statusBars).padding(top = 56.dp)) {
        SkeletonBlock(Modifier.align(Alignment.CenterHorizontally).size(240.dp))
        Spacer(Modifier.height(20.dp))
        SkeletonBlock(Modifier.padding(horizontal = 16.dp).fillMaxWidth(0.7f).height(24.dp))
        Spacer(Modifier.height(10.dp))
        SkeletonBlock(Modifier.padding(horizontal = 16.dp).fillMaxWidth(0.4f).height(14.dp))
        Spacer(Modifier.height(16.dp))
        SkeletonList(rows = 6)
    }
}

/** Where an artist's page will be: a wide picture and rows. */
@Composable
fun SkeletonArtist(modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth()) {
        SkeletonBlock(Modifier.fillMaxWidth().height(380.dp), RoundedCornerShape(0.dp))
        Spacer(Modifier.height(16.dp))
        SkeletonList(rows = 5)
    }
}

/** Tiles of the search page's wall while it loads. */
@Composable
fun SkeletonTiles(modifier: Modifier = Modifier, rows: Int = 3) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        repeat(rows) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                repeat(2) { SkeletonBlock(Modifier.weight(1f).height(96.dp), RoundedCornerShape(8.dp)) }
            }
        }
    }
}

/** A failure of one part of a page, which does not cost the rest. */
@Composable
fun SmallProblem(message: String, onRetry: () -> Unit, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    Row(modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Glyph(R.drawable.ic_circle_alert, null, tint = p.secondary, size = 18.dp)
        Spacer(Modifier.width(8.dp))
        Text(message, style = Type.small, color = p.secondary, modifier = Modifier.weight(1f))
        Text(
            "Try again", style = Type.label, color = p.text,
            modifier = Modifier.clip(RoundedCornerShape(12.dp)).tapRow(onRetry, role = Role.Button).padding(horizontal = 8.dp, vertical = 12.dp),
        )
    }
}

/** One thing on a page's own menu. */
class PageAction(@DrawableRes val icon: Int, val label: String, val detail: String? = null, val run: () -> Unit)

/** A page's own actions, for what its row has no room for. The sheet closes before the action runs. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PageMenu(title: String, entries: List<PageAction>, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = p.panel, contentColor = p.text) {
        Column(Modifier.padding(bottom = 24.dp)) {
            Text(title, style = Type.heading, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            entries.forEach { entry ->
                Row(
                    Modifier.fillMaxWidth().tapRow({ onDismiss(); entry.run() }, role = Role.Button).padding(horizontal = 20.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Glyph(entry.icon, null, tint = p.secondary)
                    Spacer(Modifier.width(18.dp))
                    Column {
                        Text(entry.label, style = Type.body)
                        if (entry.detail != null) Text(entry.detail, style = Type.small, color = p.secondary)
                    }
                }
            }
        }
    }
}

/** A list of choices with the one in use ticked, as a sort is. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun <T> ChoiceSheet(title: String, choices: List<T>, chosen: T, label: (T) -> String, onChoose: (T) -> Unit, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = p.panel, contentColor = p.text) {
        Column(Modifier.padding(bottom = 24.dp)) {
            Text(title, style = Type.heading, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            choices.forEach { choice ->
                val on = choice == chosen
                Row(
                    Modifier.fillMaxWidth().tapRow({ onChoose(choice); onDismiss() }, role = Role.RadioButton).padding(horizontal = 20.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(label(choice), style = Type.body, color = if (on) p.accent else p.text, modifier = Modifier.weight(1f))
                    if (on) Glyph(R.drawable.ic_check, "Chosen", tint = p.accent, size = 20.dp)
                }
            }
        }
    }
}

/** Asks for a line or two of text, as a new folder's name or a playlist's details. */
@Composable
fun TextDialog(
    title: String,
    action: String,
    first: String,
    firstLabel: String,
    onDone: (String, String) -> Unit,
    onDismiss: () -> Unit,
    second: String? = null,
    secondLabel: String = "",
) {
    val p = LocalPalette.current
    var one by rememberSaveable { mutableStateOf(first) }
    var two by rememberSaveable { mutableStateOf(second.orEmpty()) }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = p.overlay,
        titleContentColor = p.text,
        title = { Text(title, style = Type.heading) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(one, { one = it }, Modifier.fillMaxWidth(), label = { Text(firstLabel) }, singleLine = true)
                if (second != null) OutlinedTextField(two, { two = it }, Modifier.fillMaxWidth(), label = { Text(secondLabel) }, maxLines = 4)
            }
        },
        confirmButton = {
            TextButton({ onDone(one.trim(), two.trim()) }, enabled = one.isNotBlank()) {
                Text(action, style = Type.bodyStrong, color = if (one.isNotBlank()) p.accent else p.dim)
            }
        },
        dismissButton = { TextButton(onDismiss) { Text("Cancel", style = Type.bodyStrong, color = p.text) } },
    )
}

/** A text cut at [lines] lines with a way to read the rest, which only shows when there is more. */
@Composable
fun ExpandableText(text: String, lines: Int, modifier: Modifier = Modifier, style: androidx.compose.ui.text.TextStyle = Type.body, color: androidx.compose.ui.graphics.Color = LocalPalette.current.text) {
    val p = LocalPalette.current
    var open by rememberSaveable(text) { mutableStateOf(false) }
    var cut by remember(text) { mutableStateOf(false) }
    Column(modifier) {
        Text(
            text, style = style, color = color,
            maxLines = if (open) Int.MAX_VALUE else lines, overflow = TextOverflow.Ellipsis,
            onTextLayout = { if (!open) cut = it.hasVisualOverflow },
        )
        if (cut || open) {
            Text(
                if (open) "Show less" else "Show more", style = Type.label, color = p.text,
                modifier = Modifier.clip(RoundedCornerShape(12.dp))
                    .tapRow({ open = !open }, role = Role.Button).padding(vertical = 12.dp),
            )
        }
    }
}

/** A square tile for one of the app's own lists, where the library shows as a grid. */
@Composable
fun ShortcutTile(title: String, @DrawableRes icon: Int, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    Column(modifier.clip(RoundedCornerShape(6.dp)).tapRow(onClick, role = Role.Button, onClickLabel = "Open")) {
        Box(Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(6.dp)).background(p.accent), contentAlignment = Alignment.Center) {
            Glyph(icon, null, tint = p.onAccent, size = 32.dp)
        }
        Spacer(Modifier.height(8.dp))
        Text(title, style = Type.label, color = p.text, maxLines = 2, overflow = TextOverflow.Ellipsis)
    }
}

/** A choice of a whole selection: an icon with its word. */
class SelectionAction(@DrawableRes val icon: Int, val label: String, val run: () -> Unit)

/**
 * The bar of a list being selected from, over the page's foot. How many are
 * chosen, how to leave and choose all, and what can be done with them.
 */
@Composable
fun BoxScope.SelectionBar(count: Int, total: Int, onSelectAll: () -> Unit, onClose: () -> Unit, entries: List<SelectionAction>) {
    val p = LocalPalette.current
    Column(
        Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(horizontal = 8.dp, vertical = 8.dp)
            .clip(RoundedCornerShape(12.dp)).background(p.overlay),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            GlyphButton(R.drawable.ic_x, "Stop selecting", onClose)
            Text("$count selected", style = Type.bodyStrong, modifier = Modifier.weight(1f))
            Text(
                if (count == total) "Select none" else "Select all", style = Type.label,
                modifier = Modifier.clip(RoundedCornerShape(12.dp)).tapRow(onSelectAll, role = Role.Button).padding(horizontal = 12.dp, vertical = 14.dp),
            )
        }
        LazyRow {
            items(entries) { entry ->
                val enabled = count > 0
                Column(
                    Modifier.width(84.dp).clip(RoundedCornerShape(8.dp))
                        .tapRow(entry.run, enabled = enabled, role = Role.Button).padding(vertical = 8.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Glyph(entry.icon, null, tint = if (enabled) p.text else p.dim, size = 22.dp)
                    Text(entry.label, style = Type.tiny, color = if (enabled) p.text else p.dim, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}

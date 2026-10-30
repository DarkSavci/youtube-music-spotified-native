package dev.darksavci.spotified.ui.screens

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.darksavci.spotified.R
import dev.darksavci.spotified.data.BlockKind
import dev.darksavci.spotified.data.Blocked
import dev.darksavci.spotified.data.Changelog
import dev.darksavci.spotified.data.Licences
import dev.darksavci.spotified.data.blocked
import dev.darksavci.spotified.data.setBlocked
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

/** What is never played, each with the button that lets it play again. See crates/app/src/views/settings/blocked.rs. */
@Composable
fun BlockedScreen() {
    val actions = LocalActions.current
    val p = LocalPalette.current
    val store = actions.graph.settings
    val list by store.blocked.collectAsStateWithLifecycle(Blocked())
    Column {
        PageTitle("Blocked", actions)
        LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
            item {
                Text(
                    if (list.isEmpty) {
                        "Nothing is blocked. Long-press a song, an artist or an album and choose Block: the queue then steps over it, and radios leave it out."
                    } else {
                        "The queue steps over these, and radios leave them out."
                    },
                    style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
            for (kind in listOf(BlockKind.Artist, BlockKind.Album, BlockKind.Song)) {
                items(list.of(kind), key = { kind.noun + it.id }) { entry ->
                    // One blocked before it had a name is still to be told apart.
                    val name = entry.name.ifEmpty { entry.id }
                    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f).semantics(mergeDescendants = true) {}.padding(end = 12.dp)) {
                            Text(name, style = Type.body, maxLines = 2)
                            Text(kind.noun, style = Type.small, color = p.secondary)
                        }
                        Pill("Unblock", {
                            actions.write(done = "$name can play again") { store.setBlocked(kind, entry.id, entry.name, false) }
                        }, description = "Unblock $name")
                    }
                }
            }
        }
    }
}

/** Every release's notes, which travel with the app. */
@Composable
fun WhatsNewScreen() {
    val actions = LocalActions.current
    val p = LocalPalette.current
    val context = LocalContext.current
    val releases = remember { Changelog.read(context) }
    Column {
        PageTitle("What's new", actions)
        LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
            if (releases.isEmpty()) item { Line("The notes could not be read", "") }
            items(releases, key = { it.version }) { release ->
                Column(Modifier.fillMaxWidth()) {
                    SectionTitle(release.version)
                    if (release.date.isNotEmpty()) Text(release.date, style = Type.small, color = p.secondary, modifier = Modifier.padding(start = 16.dp))
                    if (release.note.isNotEmpty()) Text(release.note, style = Type.body, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
                    release.groups.forEach { group ->
                        Text(group.title, style = Type.bodyStrong, modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp).semantics { heading() })
                        group.items.forEach { Text("• $it", style = Type.body, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp, vertical = 3.dp)) }
                    }
                }
            }
        }
    }
}

@Composable
fun LicencesScreen() {
    val actions = LocalActions.current
    val p = LocalPalette.current
    val context = LocalContext.current
    Column {
        PageTitle("Licences", actions)
        LazyColumn(contentPadding = PaddingValues(bottom = 24.dp)) {
            items(Licences.ALL, key = { it.name }) { entry ->
                Row(
                    Modifier.fillMaxWidth().pressRow(label = "Open ${entry.name}'s page") {
                        runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(entry.url))) }
                    }.padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(entry.name, style = Type.body)
                        Text(entry.licence, style = Type.small, color = p.secondary)
                    }
                    Glyph(R.drawable.ic_external_link, null, tint = p.secondary, size = 18.dp)
                }
            }
        }
    }
}

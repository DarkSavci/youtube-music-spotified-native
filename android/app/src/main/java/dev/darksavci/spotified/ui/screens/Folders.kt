package dev.darksavci.spotified.ui.screens

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Folder
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.Change
import dev.darksavci.spotified.ui.MenuTarget
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.SkeletonList
import dev.darksavci.spotified.ui.components.SmallProblem
import dev.darksavci.spotified.ui.components.TextDialog
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

/** The move-to-folder target for an item of the library; a song is not filed. */
fun moveTargetOf(item: Item): MenuTarget.MoveToFolder? = when (item) {
    is Item.AlbumItem -> MenuTarget.MoveToFolder(dev.darksavci.spotified.api.LibraryKind.Album, item.album.id, item.title)
    is Item.PlaylistItem -> MenuTarget.MoveToFolder(dev.darksavci.spotified.api.LibraryKind.Playlist, item.playlist.id, item.title)
    is Item.ArtistItem -> MenuTarget.MoveToFolder(dev.darksavci.spotified.api.LibraryKind.Artist, item.artist.id, item.title)
    is Item.PodcastItem -> MenuTarget.MoveToFolder(dev.darksavci.spotified.api.LibraryKind.Podcast, item.podcast.id, item.title)
    else -> null
}

/** Files [target] in a folder, makes a folder for it, or takes it out of the one it is in. */
@Composable
fun FolderSheet(actions: Actions, target: MenuTarget.MoveToFolder) {
    val api = actions.graph.api
    val p = LocalPalette.current
    val close = { actions.closeMenus() }
    var naming by remember { mutableStateOf(false) }
    var attempt by remember { mutableStateOf(0) }
    val folders by produceState<Result<List<Folder>>?>(null, attempt) { value = runCatching { api.folders() } }
    val current by produceState<String?>(null) {
        value = runCatching { libraryEntry(api.library(), target.kind, target.id)?.folderId.orEmpty() }.getOrDefault("")
    }

    val move = { id: String, said: String ->
        close()
        actions.write(Change.Library, done = said) { api.organise(target.kind, target.id, folderId = id) }
    }

    Text("Move to folder", style = Type.heading, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
    Text(target.title, style = Type.small, color = p.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp))
    FolderRow(R.drawable.ic_folder_plus, "New folder", false) { naming = true }
    if (!current.isNullOrEmpty()) FolderRow(R.drawable.ic_x, "Take out of this folder", false) { move("", "Taken out of the folder") }
    val answer = folders
    when {
        answer == null -> SkeletonList(rows = 3, artSize = 0.dp)
        answer.isFailure -> SmallProblem("Couldn't load your folders.", { attempt += 1 })
        else -> answer.getOrThrow().forEach { f ->
            FolderRow(R.drawable.ic_folder, f.name, f.id == current) { if (f.id != current) move(f.id, "Moved to ${f.name}") else close() }
        }
    }

    if (naming) {
        TextDialog(
            "New folder", "Create", "", "Name",
            { name, _ ->
                naming = false
                close()
                actions.write(Change.Folders, Change.Library, done = "Moved to $name") {
                    api.organise(target.kind, target.id, folderId = api.createFolder(name))
                }
            },
            { naming = false },
        )
    }
}

@Composable
private fun FolderRow(icon: Int, label: String, here: Boolean, onClick: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxWidth().tapRow(onClick, role = Role.Button).padding(horizontal = 20.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Glyph(icon, null, tint = if (here) p.accent else p.secondary)
        Spacer(Modifier.width(18.dp))
        Text(label, style = Type.body, color = if (here) p.accent else p.text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        if (here) Glyph(R.drawable.ic_check, "In this folder", tint = p.accent, size = 20.dp)
    }
}

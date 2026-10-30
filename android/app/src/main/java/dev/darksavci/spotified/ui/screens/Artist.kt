package dev.darksavci.spotified.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Album
import dev.darksavci.spotified.api.Artist
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.StatDetail
import dev.darksavci.spotified.api.url
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.Change
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.CoverCard
import dev.darksavci.spotified.ui.components.ExpandableText
import dev.darksavci.spotified.ui.components.FadingTopBar
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.PlayButton
import dev.darksavci.spotified.ui.components.Refreshable
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.SkeletonArtist
import dev.darksavci.spotified.ui.components.SubPage
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.artColor
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.components.tapRow
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

@Composable
fun ArtistScreen(id: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("artist:$id") { actions.graph.api.artist(id) }
    SubPage(loader, skeleton = { SkeletonArtist() }) { artist ->
        Refreshable(loader) { ArtistPage(artist, actions) }
    }
}

@Composable
private fun ArtistPage(artist: Artist, actions: Actions) {
    val p = LocalPalette.current
    val prefs = PagePrefs.of(LocalContext.current)
    val top = songsShown(artist.topTracks, prefs.showMusicVideos)
    var following by remember(artist.id, artist.following) { mutableStateOf(artist.following) }
    var morePopular by remember { mutableStateOf(false) }
    val yours by produceState<StatDetail?>(null, artist.id) {
        value = runCatching { actions.graph.api.statDetail("artist", artist.id) }.getOrNull()?.takeIf { it.plays > 0 }
    }
    val (here, playing) = rememberPlayingHere(top, artist.name, actions)
    val list = rememberLazyListState()
    val shown = if (morePopular) top else top.take(5)
    val keys = remember(shown) { rowKeys(shown) }
    val selection = rememberTrackSelection()
    Box(Modifier.fillMaxSize()) {
    LazyColumn(state = list, contentPadding = PaddingValues(bottom = if (selection.active) 140.dp else 24.dp)) {
        item {
            // The artist's banner as it is, wide: asked for square, YouTube pads it with black bars.
            val banner = artist.artwork.maxByOrNull { it.width }?.url
            Box(Modifier.fillMaxWidth().height(380.dp)) {
                AsyncImage(banner, "Picture of ${artist.name}", Modifier.fillMaxSize(), contentScale = ContentScale.Crop, alignment = Alignment.TopCenter)
                Box(
                    Modifier.fillMaxSize().background(
                        Brush.verticalGradient(0f to Color.Black.copy(alpha = 0.35f), 0.25f to Color.Transparent, 0.6f to Color.Transparent, 1f to p.window),
                    ),
                )
                Text(
                    artist.name, style = Type.display.copy(fontSize = Type.display.fontSize * 1.4f), color = Color.White,
                    maxLines = 2, overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.align(Alignment.BottomStart).padding(16.dp),
                )
            }
        }
        item {
            Text(
                listOf(artist.monthlyListeners, artist.subscribers).firstOrNull { it.isNotEmpty() }.orEmpty(),
                style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp),
            )
            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    if (following) "Following" else "Follow", style = Type.label,
                    modifier = Modifier.clip(RoundedCornerShape(16.dp))
                        .border(1.dp, if (following) p.text else p.secondary, RoundedCornerShape(16.dp))
                        .tapRow({
                            val wanted = !following
                            following = wanted
                            // A follow that does not go through puts the button back and says why.
                            actions.launch {
                                runCatching { actions.graph.api.setFollowing(artist.id, wanted) }
                                    .onSuccess { actions.changed(Change.Library) }
                                    .onFailure { e ->
                                        following = !wanted
                                        actions.say(e.message ?: "That could not be saved.")
                                    }
                            }
                        }, role = Role.Button, onClickLabel = if (following) "Stop following ${artist.name}" else "Follow ${artist.name}")
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                )
                if (artist.radioId.isNotEmpty()) {
                    GlyphButton(R.drawable.ic_radio, "Radio", {
                        actions.playback.startMix(artist.radioId, artist.radioSeed, artist.radioParams, "${artist.name} radio")
                    }, tint = p.secondary)
                }
                GlyphButton(R.drawable.ic_ellipsis, "More for ${artist.name}", { actions.menuFor(Item.ArtistItem(artist)) }, tint = p.secondary)
                Spacer(Modifier.weight(1f))
                GlyphButton(R.drawable.ic_shuffle, "Shuffle", {
                    if (artist.shuffleId.isNotEmpty()) actions.playback.startMix(artist.shuffleId, artist.shuffleSeed, artist.shuffleParams, artist.name)
                    else actions.playback.shuffle(top, artist.name)
                }, tint = p.secondary)
                Spacer(Modifier.width(4.dp))
                PlayButton({ if (here) actions.playback.toggle() else actions.play(top, 0, artist.name) }, playing = playing)
            }
        }
        yours?.let { stat ->
            item {
                Text(
                    "You've played ${artist.name} ${times(stat.plays)}" + if (stat.rank > 0) " — #${stat.rank} among your artists" else "",
                    style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp),
                )
            }
        }
        if (top.isNotEmpty()) {
            item { SectionTitle("Popular") }
            itemsIndexed(shown) { i, t ->
                SelectableRow(
                    t, keys[i], selection, { actions.play(top, i, artist.name) }, { actions.menuFor(t) },
                    number = i + 1, subtitle = t.playCount.ifEmpty { null },
                )
            }
            // Folding the list while choosing from it would leave songs chosen out of sight.
            if (top.size > 5 && !(morePopular && selection.active)) {
                item {
                    Text(
                        if (morePopular) "Show less" else "See more", style = Type.label,
                        modifier = Modifier.padding(16.dp).clip(RoundedCornerShape(16.dp)).background(p.surface)
                            .tapRow({ morePopular = !morePopular }, role = Role.Button).padding(horizontal = 14.dp, vertical = 8.dp),
                    )
                }
            }
        }
        releases("Albums", artist.albums, artist.albumsMore?.let { Routes.browse(it.id, it.params, "${artist.name} — albums") }, actions)
        releases("Singles and EPs", artist.singles, artist.singlesMore?.let { Routes.browse(it.id, it.params, "${artist.name} — singles") }, actions)
        if (artist.related.isNotEmpty()) {
            item { SectionTitle("Fans also like") }
            item {
                LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    items(artist.related) { a ->
                        CoverCard(
                            a.name, "Artist", a.artwork, { actions.go(Routes.artist(a.id)) }, size = 130.dp, round = true,
                            onLongClick = { actions.menuFor(Item.ArtistItem(a)) },
                        )
                    }
                }
            }
        }
        if (artist.description.isNotEmpty()) {
            item { SectionTitle("About") }
            item {
                Column(Modifier.padding(horizontal = 16.dp).clip(RoundedCornerShape(8.dp)).background(p.surface).padding(horizontal = 16.dp).padding(top = 16.dp, bottom = 4.dp)) {
                    ExpandableText(artist.description, 8)
                    if (artist.descriptionUrl.isNotEmpty()) SourceLink(artist.descriptionUrl, actions)
                }
            }
        }
    }
    FadingTopBar(artist.name, artColor(artist.artwork), list) { actions.nav.popBackStack() }
    TrackSelectionBar(selection, shown, keys, actions)
    }
}

/** "From Wikipedia", which opens the article; the core says only where the words came from. */
@Composable
private fun SourceLink(url: String, actions: Actions) {
    val p = LocalPalette.current
    val uris = LocalUriHandler.current
    val web = url.startsWith("https://") || url.startsWith("http://")
    Row(
        Modifier.clip(RoundedCornerShape(12.dp))
            .tapRow({
                runCatching { uris.openUri(url) }.onFailure { actions.say("No app here can open that link.") }
            }, enabled = web, onClickLabel = "Open in the browser")
            .padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("From Wikipedia", style = Type.small, color = p.secondary)
        if (web) {
            Spacer(Modifier.width(4.dp))
            Glyph(R.drawable.ic_external_link, null, tint = p.secondary, size = 14.dp)
        }
    }
}

private fun LazyListScope.releases(title: String, albums: List<Album>, more: String?, actions: Actions) {
    if (albums.isEmpty()) return
    item { SectionTitle(title, action = if (more != null) "Show all" else null, onAction = more?.let { { actions.go(it) } }) }
    item {
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            items(albums) { a ->
                CoverCard(
                    a.title, listOf(a.year, a.kind).filter { it.isNotEmpty() }.joinToString(" • "), a.artwork, { actions.go(Routes.album(a.id)) },
                    onLongClick = { actions.menuFor(Item.AlbumItem(a)) },
                )
            }
        }
    }
}

package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.ui.components.artColor
import dev.darksavci.spotified.ui.components.FadingTopBar
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import coil3.compose.AsyncImage
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Album
import dev.darksavci.spotified.api.Artist
import dev.darksavci.spotified.api.StatDetail
import dev.darksavci.spotified.api.url
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.CoverCard
import dev.darksavci.spotified.ui.components.GlyphButton
import dev.darksavci.spotified.ui.components.Loaded
import dev.darksavci.spotified.ui.components.PlayButton
import dev.darksavci.spotified.ui.components.SectionTitle
import dev.darksavci.spotified.ui.components.TrackRow
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

@Composable
fun ArtistScreen(id: String) {
    val actions = LocalActions.current
    val loader = rememberLoader("artist:$id") { actions.graph.api.artist(id) }
    Loaded(loader) { artist -> ArtistPage(artist, actions) }
}

@Composable
private fun ArtistPage(artist: Artist, actions: Actions) {
    val p = LocalPalette.current
    val projection by actions.graph.playback.projection.collectAsStateWithLifecycle()
    var following by remember(artist.id) { mutableStateOf(artist.following) }
    var morePopular by remember { mutableStateOf(false) }
    val yours by produceState<StatDetail?>(null, artist.id) {
        value = runCatching { actions.graph.api.statDetail("artist", artist.id) }.getOrNull()?.takeIf { it.plays > 0 }
    }
    val list = rememberLazyListState()
    Box {
    LazyColumn(state = list, contentPadding = PaddingValues(bottom = 24.dp)) {
        item {
            // The artist's banner as it is, wide: asked for square, YouTube pads it with black bars.
            val banner = artist.artwork.maxByOrNull { it.width }?.url
            Box(Modifier.fillMaxWidth().height(380.dp)) {
                AsyncImage(banner, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop, alignment = Alignment.TopCenter)
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
                        .clickable {
                            following = !following
                            actions.launch { actions.graph.api.setFollowing(artist.id, following) }
                        }
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                )
                if (artist.radioId.isNotEmpty()) {
                    GlyphButton(R.drawable.ic_radio, "Radio", {
                        actions.playback.startMix(artist.radioId, artist.radioSeed, artist.radioParams, "${artist.name} radio")
                    }, tint = p.secondary)
                }
                GlyphButton(R.drawable.ic_share_2, "Share", { actions.share(artist.name, Actions.artistLink(artist.id)) }, tint = p.secondary)
                Spacer(Modifier.weight(1f))
                GlyphButton(R.drawable.ic_shuffle, "Shuffle", {
                    if (artist.shuffleId.isNotEmpty()) actions.playback.startMix(artist.shuffleId, artist.shuffleSeed, artist.shuffleParams, artist.name)
                    else actions.playback.shuffle(artist.topTracks, artist.name)
                }, tint = p.secondary)
                Spacer(Modifier.width(4.dp))
                PlayButton({ actions.play(artist.topTracks, 0, artist.name) })
            }
        }
        yours?.let { stat ->
            item {
                Text(
                    "You've played ${artist.name} ${stat.plays} times" + if (stat.rank > 0) " — #${stat.rank} among your artists" else "",
                    style = Type.small, color = p.secondary, modifier = Modifier.padding(horizontal = 16.dp),
                )
            }
        }
        if (artist.topTracks.isNotEmpty()) {
            item { SectionTitle("Popular") }
            val shown = if (morePopular) artist.topTracks else artist.topTracks.take(5)
            itemsIndexed(shown) { i, t ->
                TrackRow(
                    t, { actions.play(artist.topTracks, i, artist.name) }, { actions.menuFor(t) },
                    playing = projection.state.current?.id == t.id, number = i + 1, subtitle = t.playCount.ifEmpty { null },
                )
            }
            if (artist.topTracks.size > 5) {
                item {
                    Text(
                        if (morePopular) "Show less" else "See more", style = Type.label,
                        modifier = Modifier.padding(16.dp).clip(RoundedCornerShape(16.dp)).background(p.surface)
                            .clickable { morePopular = !morePopular }.padding(horizontal = 14.dp, vertical = 8.dp),
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
                        CoverCard(a.name, "Artist", a.artwork, { actions.go(Routes.artist(a.id)) }, size = 130.dp, round = true)
                    }
                }
            }
        }
        if (artist.description.isNotEmpty()) {
            item { SectionTitle("About") }
            item {
                Column(Modifier.padding(horizontal = 16.dp).clip(RoundedCornerShape(8.dp)).background(p.surface).padding(16.dp)) {
                    Text(artist.description, style = Type.body, maxLines = 8, overflow = TextOverflow.Ellipsis)
                    if (artist.descriptionUrl.isNotEmpty()) Text("From Wikipedia", style = Type.small, color = p.secondary)
                }
            }
        }
    }
    FadingTopBar(artist.name, artColor(artist.artwork), list) { actions.nav.popBackStack() }
    }
}

private fun LazyListScope.releases(title: String, albums: List<Album>, more: String?, actions: Actions) {
    if (albums.isEmpty()) return
    item { SectionTitle(title, action = if (more != null) "Show all" else null, onAction = more?.let { { actions.go(it) } }) }
    item {
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            items(albums) { a ->
                CoverCard(a.title, listOf(a.year, a.kind).filter { it.isNotEmpty() }.joinToString(" • "), a.artwork, { actions.go(Routes.album(a.id)) })
            }
        }
    }
}

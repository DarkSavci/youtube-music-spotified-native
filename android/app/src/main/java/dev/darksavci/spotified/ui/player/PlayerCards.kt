package dev.darksavci.spotified.ui.player

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
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
import coil3.compose.AsyncImage
import dev.darksavci.spotified.api.Artist
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.api.url
import dev.darksavci.spotified.ui.Actions
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.theme.Type

/** What plays, past the lyrics, as Spotify's player scrolls on: more of the artist, then the artist. */
@Composable
fun PlayerCards(track: Track, actions: Actions) {
    val artistId = track.artists.firstOrNull { it.id.isNotEmpty() }?.id ?: return
    val artist by produceState<Artist?>(null, artistId) {
        value = runCatching { actions.graph.api.artist(artistId) }.getOrNull()
    }
    val a = artist ?: return
    Spacer(Modifier.height(12.dp))
    Explore(track, a, actions)
    Spacer(Modifier.height(12.dp))
    AboutArtist(a, actions)
}

/** Ways on from this song: the artist's songs, and a radio from it. */
@Composable
private fun Explore(track: Track, artist: Artist, actions: Actions) {
    Card {
        Text("Explore ${artist.name}", style = Type.bodyStrong, color = Color.White)
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Tile("Songs by ${artist.name}", artist.topTracks.firstOrNull()?.artwork ?: track.artwork, Modifier.weight(1f)) {
                actions.nowPlayingOpen = false
                actions.go(Routes.artist(artist.id))
            }
            Tile("Radio from this song", track.artwork, Modifier.weight(1f)) {
                actions.playback.startRadio(track)
            }
            artist.related.firstOrNull()?.let { similar ->
                Tile("Similar: ${similar.name}", similar.artwork, Modifier.weight(1f)) {
                    actions.nowPlayingOpen = false
                    actions.go(Routes.artist(similar.id))
                }
            }
        }
    }
}

@Composable
private fun Tile(title: String, artwork: List<Artwork>, modifier: Modifier, onClick: () -> Unit) {
    Box(modifier.aspectRatio(0.66f).clip(RoundedCornerShape(8.dp)).clickable(onClick = onClick)) {
        AsyncImage(artwork.url(300), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.4f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.8f))))
        Text(
            title, style = Type.label, color = Color.White, maxLines = 3, overflow = TextOverflow.Ellipsis,
            modifier = Modifier.align(Alignment.BottomStart).padding(8.dp),
        )
    }
}

/** The artist's picture, how many listen, following them, and a little of who they are. */
@Composable
private fun AboutArtist(artist: Artist, actions: Actions) {
    var following by remember(artist.id) { mutableStateOf(artist.following) }
    var expanded by remember(artist.id) { mutableStateOf(false) }
    Column(
        Modifier.padding(horizontal = 16.dp).fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Color.White.copy(alpha = 0.08f))
            .clickable {
                actions.nowPlayingOpen = false
                actions.go(Routes.artist(artist.id))
            },
    ) {
        Box(Modifier.fillMaxWidth().height(200.dp)) {
            AsyncImage(
                artist.artwork.maxByOrNull { it.width }?.url, null, Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop, alignment = Alignment.TopCenter,
            )
            Text("About the artist", style = Type.bodyStrong, color = Color.White, modifier = Modifier.padding(16.dp))
        }
        Column(Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(artist.name, style = Type.heading, color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    val listeners = listOf(artist.monthlyListeners, artist.subscribers).firstOrNull { it.isNotEmpty() }
                    if (listeners != null) Text(listeners, style = Type.small, color = Color.White.copy(alpha = 0.7f))
                }
                Text(
                    if (following) "Following" else "Follow", style = Type.label, color = Color.White,
                    modifier = Modifier.clip(RoundedCornerShape(16.dp)).border(1.dp, Color.White.copy(alpha = 0.6f), RoundedCornerShape(16.dp))
                        .clickable {
                            following = !following
                            actions.launch { actions.graph.api.setFollowing(artist.id, following) }
                        }
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
            if (artist.description.isNotEmpty()) {
                Spacer(Modifier.height(10.dp))
                Text(
                    artist.description, style = Type.small, color = Color.White.copy(alpha = 0.85f),
                    maxLines = if (expanded) Int.MAX_VALUE else 3, overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.clickable { expanded = !expanded },
                )
            }
        }
    }
}

@Composable
private fun Card(content: @Composable () -> Unit) {
    Column(Modifier.padding(horizontal = 16.dp).fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Color.White.copy(alpha = 0.08f)).padding(16.dp)) {
        content()
    }
}

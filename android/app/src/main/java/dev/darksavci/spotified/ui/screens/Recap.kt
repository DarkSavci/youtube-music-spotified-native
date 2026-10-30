package dev.darksavci.spotified.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
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
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Artwork
import dev.darksavci.spotified.api.channelId
import dev.darksavci.spotified.api.cover
import dev.darksavci.spotified.api.url
import dev.darksavci.spotified.ui.LocalActions
import dev.darksavci.spotified.ui.components.Art
import dev.darksavci.spotified.ui.components.Chip
import dev.darksavci.spotified.ui.components.Empty
import dev.darksavci.spotified.ui.components.Loaded
import dev.darksavci.spotified.ui.components.Pill
import dev.darksavci.spotified.ui.components.SkeletonBlock
import dev.darksavci.spotified.ui.components.rememberLoader
import dev.darksavci.spotified.ui.theme.Type
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import kotlinx.coroutines.launch

/** Each card's two colours, run corner to corner. */
private val RECAP_COLOURS = listOf(
    Color(0xFFFF0033) to Color(0xFF5A0A2A),
    Color(0xFF1DB9C3) to Color(0xFF16324F),
    Color(0xFFFFB300) to Color(0xFFB3261E),
    Color(0xFF8E44EC) to Color(0xFF1F1147),
)

/** A calendar year as a few cards to swipe or tap through and share, from the core's own counts. */
@Composable
fun RecapScreen() {
    val actions = LocalActions.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val today = remember { LocalDate.now() }
    var year by rememberSaveable { mutableIntStateOf(firstRecapYear(today)) }
    val span = remember(year) { recapSpan(year, today, ZoneId.systemDefault(), Instant.now()) }
    val loader = rememberLoader("recap:$year") { actions.graph.api.statsBetween(span.from, span.to, span.days) }
    Column(Modifier.fillMaxSize()) {
        PageTitle("Your year", actions)
        Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            recapYears(today).forEach { y -> Chip(y.toString(), y == year, { year = y }) }
        }
        Box(Modifier.weight(1f)) {
            Loaded(loader, keep = true, skeleton = { SkeletonBlock(Modifier.fillMaxSize().padding(24.dp), RoundedCornerShape(20.dp)) }) { stats ->
                val cards = recapCards(stats, span)
                if (cards.isEmpty()) {
                    Empty("Nothing from $year" to "Your year is made from what you play in this app, on this account.")
                    return@Loaded
                }
                key(year) {
                    val pager = rememberPagerState { cards.size }
                    var sharing by remember { mutableStateOf(false) }
                    Column(Modifier.fillMaxSize()) {
                        HorizontalPager(pager, Modifier.weight(1f).fillMaxWidth(), contentPadding = PaddingValues(horizontal = 24.dp, vertical = 12.dp), pageSpacing = 16.dp) { page ->
                            val (from, to) = RECAP_COLOURS[page % RECAP_COLOURS.size]
                            val go = { to: Int -> if (to in cards.indices) scope.launch { pager.animateScrollToPage(to) } }
                            Box(
                                Modifier.fillMaxSize().clip(RoundedCornerShape(20.dp))
                                    .background(Brush.linearGradient(listOf(from, to)))
                                    // Big soft circles, Wrapped's way of making a card a poster.
                                    .drawBehind {
                                        drawCircle(Color.White.copy(alpha = 0.08f), radius = size.width * 0.75f, center = Offset(size.width * 1.05f, size.height * 0.12f))
                                        drawCircle(Color.Black.copy(alpha = 0.12f), radius = size.width * 0.6f, center = Offset(-size.width * 0.1f, size.height * 0.95f))
                                    }
                                    // As stories go: the left third goes back, the rest goes on.
                                    .pointerInput(cards.size) {
                                        detectTapGestures { at -> go(if (at.x < size.width / 3f) page - 1 else page + 1) }
                                    }
                                    .semantics {
                                        customActions = listOf(
                                            CustomAccessibilityAction("Next card") { go(page + 1); true },
                                            CustomAccessibilityAction("Previous card") { go(page - 1); true },
                                        )
                                    },
                            ) {
                                CardBody(cards[page])
                                Column(Modifier.fillMaxWidth().padding(16.dp)) {
                                    // Story bars: which card of how many.
                                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                                        repeat(cards.size) { i ->
                                            Box(
                                                Modifier.weight(1f).height(3.dp).clip(RoundedCornerShape(2.dp))
                                                    .background(Color.White.copy(alpha = if (i <= page) 0.95f else 0.3f)),
                                            )
                                        }
                                    }
                                    Spacer(Modifier.height(12.dp))
                                    Text(labelOf(span), style = Type.label.copy(letterSpacing = 2.sp), color = Color.White.copy(alpha = 0.85f))
                                }
                            }
                        }
                        Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.Center) {
                            Pill(if (sharing) "Making the picture…" else "Share this card", {
                                if (sharing) return@Pill
                                sharing = true
                                val page = pager.currentPage
                                val card = cards[page]
                                val photo = (card as? RecapCard.Feature)?.takeIf { it.round }?.let { photoAddress(it.artistId) }
                                scope.launch {
                                    runCatching { RecapImage.share(context, card, labelOf(span), RECAP_COLOURS[page % RECAP_COLOURS.size], photo) }
                                        .onFailure { actions.say("The picture could not be made.") }
                                    sharing = false
                                }
                            }, icon = R.drawable.ic_share_2, enabled = !sharing)
                        }
                    }
                }
            }
        }
    }
}

private fun labelOf(span: RecapSpan) = if (span.running) "YOUR ${span.year} SO FAR" else "YOUR ${span.year}"

private fun photoAddress(artistId: String): String? = channelId(artistId)?.let { artistPhotos[it] }?.url(720)

@Composable
private fun CardBody(card: RecapCard) {
    when (card) {
        is RecapCard.Time -> Big(card.value, card.caption)
        is RecapCard.Feature -> {
            val art = if (card.round) artistPhoto(card.artistId, card.title) ?: cover(card.art) else cover(card.art)
            Feature(card.title, card.subtitle, art, card.caption, card.round)
        }
        is RecapCard.Top -> Column(Modifier.fillMaxSize().padding(horizontal = 24.dp), verticalArrangement = Arrangement.Center) {
            Text(card.title, style = Type.display, color = Color.White)
            Spacer(Modifier.height(24.dp))
            card.rows.forEachIndexed { i, t ->
                Row(Modifier.padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("${i + 1}", style = Type.display, color = Color.White.copy(alpha = 0.55f), modifier = Modifier.width(44.dp))
                    Art(cover(t.art), 52.dp)
                    Spacer(Modifier.width(14.dp))
                    Column {
                        Text(t.title, style = Type.bodyStrong.copy(fontSize = 17.sp), color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(t.artist, style = Type.small, color = Color.White.copy(alpha = 0.75f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}

/** One number as the whole card, poster-size. */
@Composable
private fun Big(value: String, caption: String) {
    Column(Modifier.fillMaxSize().padding(horizontal = 24.dp), verticalArrangement = Arrangement.Center) {
        Text("You listened for", style = Type.heading, color = Color.White.copy(alpha = 0.85f))
        Text(value, style = Type.display.copy(fontSize = 64.sp, lineHeight = 68.sp, fontWeight = FontWeight.Black), color = Color.White)
        Spacer(Modifier.height(8.dp))
        Text(caption, style = Type.heading, color = Color.White.copy(alpha = 0.85f))
    }
}

/** A song or artist of the year: the picture large, the name under it. */
@Composable
private fun Feature(title: String, subtitle: String, artwork: List<Artwork>, caption: String, round: Boolean = false) {
    Column(Modifier.fillMaxSize().padding(horizontal = 24.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(caption, style = Type.heading, color = Color.White, textAlign = TextAlign.Center)
        Spacer(Modifier.height(24.dp))
        val shape = if (round) CircleShape else RoundedCornerShape(8.dp)
        Art(artwork, 220.dp, Modifier.shadow(24.dp, shape), shape = shape)
        Spacer(Modifier.height(24.dp))
        Text(title, style = Type.display, color = Color.White, maxLines = 2, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center)
        if (subtitle.isNotEmpty()) Text(subtitle, style = Type.heading, color = Color.White.copy(alpha = 0.8f), textAlign = TextAlign.Center)
    }
}

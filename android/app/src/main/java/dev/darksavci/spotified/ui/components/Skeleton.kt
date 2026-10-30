package dev.darksavci.spotified.ui.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.darksavci.spotified.ui.theme.LocalPalette

/** A soft light passing across what is still loading. Clip the element first. */
@Composable
fun Modifier.shimmer(): Modifier {
    val p = LocalPalette.current
    val sweep = rememberInfiniteTransition(label = "shimmer").animateFloat(
        -0.5f, 1.5f, infiniteRepeatable(tween(1300, easing = LinearEasing)), label = "sweep",
    )
    // Read while drawing, so only the drawing repeats, not the composition.
    return drawBehind {
        val at = size.width * sweep.value
        val band = size.width / 2
        drawRect(Brush.linearGradient(listOf(p.surface, p.surfaceActive, p.surface), Offset(at - band, 0f), Offset(at + band, 0f)))
    }
}

/** One shape of a placeholder, for building a page's own skeleton. */
@Composable
fun SkeletonBlock(modifier: Modifier = Modifier, shape: Shape = RoundedCornerShape(4.dp)) {
    Box(modifier.clip(shape).shimmer())
}

/** Where a [TrackRow] or, with a larger art, an [EntityRow] will be. [width] is how much of the row its title takes. */
@Composable
fun SkeletonRow(modifier: Modifier = Modifier, artSize: Dp = 48.dp, round: Boolean = false, width: Float = 0.6f) {
    Row(
        modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (artSize > 0.dp) {
            SkeletonBlock(Modifier.size(artSize), if (round) CircleShape else RoundedCornerShape(4.dp))
            Spacer(Modifier.width(12.dp))
        }
        Column(Modifier.weight(1f)) {
            SkeletonBlock(Modifier.fillMaxWidth(width).height(14.dp))
            Spacer(Modifier.height(8.dp))
            SkeletonBlock(Modifier.fillMaxWidth(width * 0.6f).height(11.dp))
        }
    }
}

/** A column of [SkeletonRow]s, for a page of rows while it loads. */
@Composable
fun SkeletonList(modifier: Modifier = Modifier, rows: Int = 8, artSize: Dp = 48.dp, round: Boolean = false) {
    Column(modifier.clearAndSetSemantics { contentDescription = "Loading" }) {
        repeat(rows) { i -> SkeletonRow(artSize = artSize, round = round, width = WIDTHS[i % WIDTHS.size]) }
    }
}

/** Where a [CoverCard] will be. */
@Composable
fun SkeletonCard(modifier: Modifier = Modifier, size: Dp = 150.dp, round: Boolean = false) {
    Column(modifier.width(size)) {
        SkeletonBlock(Modifier.size(size), if (round) CircleShape else RoundedCornerShape(6.dp))
        Spacer(Modifier.height(8.dp))
        SkeletonBlock(Modifier.fillMaxWidth(0.8f).height(13.dp))
        Spacer(Modifier.height(6.dp))
        SkeletonBlock(Modifier.fillMaxWidth(0.5f).height(11.dp))
    }
}

/** Where a titled [ShelfRow] will be. */
@Composable
fun SkeletonShelf(modifier: Modifier = Modifier, cards: Int = 4, size: Dp = 150.dp) {
    Column(modifier.fillMaxWidth().clearAndSetSemantics { contentDescription = "Loading" }) {
        SkeletonBlock(Modifier.padding(start = 16.dp, top = 24.dp, bottom = 12.dp).width(160.dp).height(20.dp))
        Row(
            Modifier.horizontalScroll(rememberScrollState(), enabled = false).padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) { repeat(cards) { SkeletonCard(size = size) } }
    }
}

/** A page of shelves while it loads, as Home and Browse are. */
@Composable
fun SkeletonShelves(modifier: Modifier = Modifier, shelves: Int = 3) {
    Column(modifier) { repeat(shelves) { SkeletonShelf() } }
}

private val WIDTHS = listOf(0.62f, 0.8f, 0.5f, 0.72f, 0.58f, 0.86f)

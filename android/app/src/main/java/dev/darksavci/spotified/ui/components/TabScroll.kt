package dev.darksavci.spotified.ui.components

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import dev.darksavci.spotified.ui.LocalActions
import kotlinx.coroutines.launch

/**
 * Says what a tab's root scrolls, so that tapping the tab again takes it to
 * the top. [tab] is `Routes.HOME`, `Routes.SEARCH` or `Routes.LIBRARY`. Call it
 * for whichever list is showing; it is forgotten when it leaves the page.
 */
@Composable
fun TabScroll(tab: String, list: LazyListState) = Follow(tab) { list.animateScrollToItem(0) }

@Composable
fun TabScroll(tab: String, grid: LazyGridState) = Follow(tab) { grid.animateScrollToItem(0) }

@Composable
fun TabScroll(tab: String, scroll: ScrollState) = Follow(tab) { scroll.animateScrollTo(0) }

@Composable
private fun Follow(tab: String, toTop: suspend () -> Unit) {
    val actions = LocalActions.current
    val scope = rememberCoroutineScope()
    val latest by rememberUpdatedState(toTop)
    DisposableEffect(actions, tab) {
        val scroll = { scope.launch { latest() }; Unit }
        actions.followTabScroll(tab, scroll)
        onDispose { actions.forgetTabScroll(tab, scroll) }
    }
}

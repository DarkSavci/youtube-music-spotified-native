package dev.darksavci.spotified.ui

import dev.darksavci.spotified.ui.components.Load
import dev.darksavci.spotified.ui.components.Loader
import dev.darksavci.spotified.ui.components.ScrubIntent
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class FoundationTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

    @After
    fun tearDown() = scope.cancel()

    // ---------- tabs ----------

    @Test
    fun aTabOnAPageUnderItGoesBackToItsRoot() {
        assertEquals(TabTap.ToRoot, tabTap(Routes.LIBRARY, Routes.LIBRARY, Routes.ALBUM))
        assertEquals(TabTap.ToRoot, tabTap(Routes.SEARCH, Routes.SEARCH, Routes.ARTIST))
    }

    @Test
    fun homeHasAWayBackToItsRootToo() {
        assertEquals(TabTap.ToRoot, tabTap(Routes.HOME, Routes.HOME, Routes.ALBUM))
    }

    @Test
    fun aTabAtItsRootGoesToItsTop() {
        assertEquals(TabTap.ToTop, tabTap(Routes.HOME, Routes.HOME, Routes.HOME))
        assertEquals(TabTap.ToTop, tabTap(Routes.LIBRARY, Routes.LIBRARY, Routes.LIBRARY))
        assertEquals(TabTap.ToTop, tabTap(Routes.SEARCH, Routes.SEARCH, Routes.SEARCH_ROUTE))
    }

    @Test
    fun onlyATabThatCanBeRestoredKeepsItsPages() {
        assertEquals(TabTap.Switch(keepLeft = true), tabTap(Routes.HOME, Routes.LIBRARY, Routes.ALBUM))
        assertEquals(TabTap.Switch(keepLeft = true), tabTap(Routes.LIBRARY, Routes.SEARCH, Routes.SEARCH_ROUTE))
        assertEquals(TabTap.Switch(keepLeft = false), tabTap(Routes.LIBRARY, Routes.HOME, Routes.ALBUM))
    }

    // ---------- changes ----------

    @Test
    fun aChangeMovesOnlyTheVersionsThatFollowIt() {
        val library = setOf<Change>(Change.Library)
        val liked = setOf<Change>(Change.Liked)
        val (beforeLibrary, beforeLiked) = Changes.version(library) to Changes.version(liked)
        Changes.say(Change.Library)
        assertEquals(beforeLibrary + 1, Changes.version(library))
        assertEquals(beforeLiked, Changes.version(liked))
    }

    @Test
    fun everythingMovesEveryFollower() {
        val liked = setOf<Change>(Change.Liked)
        val before = Changes.version(liked)
        Changes.say(Change.Everything)
        assertEquals(before + 1, Changes.version(liked))
    }

    @Test
    fun aPlaylistIsTheSameWithOrWithoutItsBrowsePrefix() {
        assertEquals(Change.Playlist("VLabc"), Change.Playlist("abc"))
        val page = setOf<Change>(Change.Playlist("abc"))
        val before = Changes.version(page)
        Changes.say(Change.Playlist("VLabc"))
        assertNotEquals(before, Changes.version(page))
    }

    // ---------- loading ----------

    @Test
    fun aQuietReloadKeepsTheContentWhileItLoads() = runBlocking {
        val next = CompletableDeferred<String>()
        var calls = 0
        val loader = Loader({ if (calls++ == 0) "old" else next.await() }, scope)
        assertEquals(Load.Ready("old"), loader.state.value)

        loader.reload(quietly = true)
        assertEquals(Load.Ready("old"), loader.state.value)
        assertTrue(loader.refreshing.value)

        next.complete("new")
        assertEquals(Load.Ready("new"), loader.state.value)
        assertFalse(loader.refreshing.value)
    }

    @Test
    fun aFailedQuietReloadKeepsTheContentAndSaysSo() = runBlocking {
        var calls = 0
        val loader = Loader({ if (calls++ == 0) "old" else error("no network") }, scope)
        val said = CompletableDeferred<String>()
        scope.launch { said.complete(loader.failures.first()) }

        loader.reload(quietly = true)

        assertEquals(Load.Ready("old"), loader.state.value)
        assertFalse(loader.refreshing.value)
        assertEquals("Could not refresh: no network", withTimeout(1000) { said.await() })
    }

    @Test
    fun aFailedLoudReloadShowsTheProblem() {
        var calls = 0
        val loader = Loader({ if (calls++ == 0) "old" else error("no network") }, scope)
        loader.reload()
        assertEquals(Load.Failed("no network"), loader.state.value)
    }

    @Test
    fun aReplacedLoadHasNoSay() {
        val slow = CompletableDeferred<String>()
        var calls = 0
        val loader = Loader({ if (calls++ == 0) slow.await() else "new" }, scope)
        loader.reload()
        assertEquals(Load.Ready("new"), loader.state.value)
        slow.complete("old")
        assertEquals(Load.Ready("new"), loader.state.value)
    }

    @Test
    fun aCancelledLoadIsNotAFailure() {
        val slow = CompletableDeferred<String>()
        val parent = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        val loader = Loader({ slow.await() }, parent)
        parent.cancel(CancellationException("page gone"))
        assertEquals(Load.Pending, loader.state.value)
    }

    @Test
    fun aTimeoutInsideAFetchIsAFailureNotACancellation() {
        val loader = Loader<String>({ withTimeout(1) { CompletableDeferred<String>().await() } }, CoroutineScope(SupervisorJob() + Dispatchers.Default))
        val state = runBlocking { withTimeout(5000) { loader.state.first { it !is Load.Pending } } }
        assertTrue(state is Load.Failed)
    }

    @Test
    fun aFollowedChangeLoadsAgainBehindTheContentOnlyWhenItMoved() {
        var calls = 0
        val loader = Loader({ ++calls }, scope).seenAt(5)
        loader.follow(5)
        assertEquals(1, calls)
        loader.follow(6)
        assertEquals(2, calls)
        assertEquals(Load.Ready(2), loader.state.value)
        assertFalse(loader.refreshing.value)
    }

    // ---------- sliders ----------

    @Test
    fun aSmallMoveDecidesNothing() {
        assertEquals(ScrubIntent.Verdict.Undecided, ScrubIntent(10f).moved(4f, 4f))
    }

    @Test
    fun aVerticalMoveIsAScrollAndNeverAScrub() {
        val intent = ScrubIntent(10f)
        assertEquals(ScrubIntent.Verdict.Undecided, intent.moved(1f, 6f))
        assertEquals(ScrubIntent.Verdict.Scroll, intent.moved(1f, 6f))
    }

    @Test
    fun aHorizontalMoveIsAScrub() {
        val intent = ScrubIntent(10f)
        assertEquals(ScrubIntent.Verdict.Undecided, intent.moved(-6f, 1f))
        assertEquals(ScrubIntent.Verdict.Scrub, intent.moved(-6f, 1f))
    }

    @Test
    fun aDiagonalBeyondTheSlopGoesToTheLargerSide() {
        assertEquals(ScrubIntent.Verdict.Scroll, ScrubIntent(10f).moved(12f, 20f))
        assertEquals(ScrubIntent.Verdict.Scrub, ScrubIntent(10f).moved(20f, 12f))
    }
}

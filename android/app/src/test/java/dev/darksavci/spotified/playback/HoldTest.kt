package dev.darksavci.spotified.playback

import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.api.Projection
import dev.darksavci.spotified.api.Target
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class HoldTest {
    @Test
    fun aMomentsLossDoesNotSoftenAHoldTheListenerHasToEnd() {
        assertEquals(Hold.ForAMoment, null.and(Hold.ForAMoment))
        assertEquals(Hold.UntilAsked, Hold.ForAMoment.and(Hold.UntilAsked))
        assertEquals(Hold.UntilAsked, Hold.UntilAsked.and(Hold.ForAMoment))
    }

    @Test
    fun focusComingBackEndsOnlyAHoldFromLosingIt() {
        assertNull(Hold.ForAMoment.afterFocusBack())
        assertEquals(Hold.UntilAsked, Hold.UntilAsked.afterFocusBack())
    }

    @Test
    fun aHeldPhoneShowsAsPausedAndTheRestIsAsTheCoreSaid() {
        val p = Projection(target = Target(epoch = 3, videoId = "a", playing = true), followingRoom = true)
        val held = p.heldBack()
        assertFalse(held.target.playing)
        assertEquals("a", held.target.videoId)
        assertTrue(held.followingRoom)
    }

    private fun FocusBook.holding() = apply {
        want(true)
        held = true
    }

    @Test
    fun playThenPauseAfterATransientLossDoesNotRestartWhenFocusComesBack() {
        val book = FocusBook().holding()
        book.pausedForTransient = true
        assertEquals(FocusBook.Step.Nothing, book.want(false))

        book.want(true)
        assertEquals(FocusBook.Step.Abandon, book.want(false))

        assertFalse(book.regained())
    }

    @Test
    fun aTransientLossCarriesOnWhenFocusComesBack() {
        val book = FocusBook().holding()
        book.pausedForTransient = true
        // The projection still on its way as the pause goes out says nothing new.
        book.want(true)
        book.want(false)
        assertTrue(book.regained())
        assertFalse(book.regained())
    }

    @Test
    fun focusIsAskedForWhenWantedAndNotHeld() {
        val book = FocusBook()
        assertEquals(FocusBook.Step.Request, book.want(true))
        book.held = true
        assertEquals(FocusBook.Step.Nothing, book.want(true))
        assertEquals(FocusBook.Step.Abandon, book.want(false))
    }

    @Test
    fun workRunsInOrderEachFinishedBeforeTheNext() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        val line = InOrder(scope)
        val seen = mutableListOf<Int>()
        val slowLetGo = CompletableDeferred<Unit>()
        val done = CompletableDeferred<Unit>()
        line.post {
            slowLetGo.await()
            seen += 1
        }
        repeat(50) { i -> line.post { seen += i + 2 } }
        line.post { done.complete(Unit) }
        // The poster is not kept waiting by the slow one, and nothing overtakes it.
        assertTrue(seen.isEmpty())
        slowLetGo.complete(Unit)
        withTimeout(5000) { done.await() }
        assertEquals((1..51).toList(), seen)
        scope.cancel()
    }

    @Test
    fun aFailureDoesNotStopTheLine() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        val errors = mutableListOf<Exception>()
        val line = InOrder(scope) { errors += it }
        val done = CompletableDeferred<Unit>()
        line.post { error("no") }
        line.post { done.complete(Unit) }
        withTimeout(5000) { done.await() }
        assertEquals(1, errors.size)
        scope.cancel()
    }

    @Test
    fun clearingTravelsByTheNamesTheCoreKnows() {
        assertEquals("clear_upcoming", Command.ClearUpcoming.wire().getValue("Kind").jsonPrimitive.content)
        assertEquals("restore_upcoming", Command.RestoreUpcoming.wire().getValue("Kind").jsonPrimitive.content)
    }
}

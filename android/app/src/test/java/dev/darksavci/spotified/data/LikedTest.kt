package dev.darksavci.spotified.data

import dev.darksavci.spotified.api.ApiError
import java.io.IOException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class LikedTest {
    private var account = "first"
    private var answer: suspend () -> List<String> = { emptyList() }
    private val liked = Liked(
        fetch = { answer() },
        write = { _, _ -> },
        account = { account },
        scope = CoroutineScope(Dispatchers.Unconfined),
    )

    private fun read(vararg ids: String) {
        answer = { ids.toList() }
        liked.refresh()
    }

    @Test
    fun aNetworkFailureLeavesTheAccountsLikes() {
        read("a", "b")
        answer = { throw IOException("no network") }
        liked.refresh()
        assertEquals(setOf("a", "b"), liked.ids.value)
    }

    @Test
    fun aFailedReadAfterASwitchShowsNoneOfTheOldAccountsLikes() {
        read("a", "b")
        account = "second"
        answer = { throw IOException("no network") }
        liked.refresh()
        assertTrue(liked.ids.value.isEmpty())
    }

    @Test
    fun anotherAccountsLikesGoAtOnceWhileItsOwnAreRead() {
        read("a")
        account = "second"
        val slow = CompletableDeferred<List<String>>()
        answer = { slow.await() }
        liked.refresh()
        assertTrue(liked.ids.value.isEmpty())
        slow.complete(listOf("z"))
        assertEquals(setOf("z"), liked.ids.value)
    }

    @Test
    fun signingOutOfTheSameAccountEmptiesThem() {
        read("a")
        answer = { throw ApiError.SignedOut }
        liked.refresh()
        assertTrue(liked.ids.value.isEmpty())
    }

    @Test
    fun aReadForTheAccountLeftIsNotShownAfterTheSwitch() {
        val slow = CompletableDeferred<List<String>>()
        answer = { slow.await() }
        liked.refresh()
        account = "second"
        read("z")
        slow.complete(listOf("a"))
        assertEquals(setOf("z"), liked.ids.value)
    }
}

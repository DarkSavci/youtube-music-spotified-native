package dev.darksavci.spotified.together

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ClientTest {
    private fun options(enter: Enter) = Options("ws://localhost:8766", "Ada", "https://yt3.ggpht.com/ada=s88", enter)

    private fun JsonObject.text(name: String) = this[name]!!.jsonPrimitive.content

    @Test
    fun aNewArrivalCreatesOrJoinsAndAReturningOneResumes() {
        val create = entrance(options(Enter.Create(Mode.Listen, "Friday")), Seat())
        assertEquals("create", create.text("type"))
        assertEquals("listen", create.text("mode"))
        assertEquals("Ada", create["profile"]!!.jsonObject.text("name"))
        assertEquals("https://yt3.ggpht.com/ada=s88", create["profile"]!!.jsonObject.text("avatar"))

        val join = options(Enter.Join("01234567"))
        assertEquals("01234567", entrance(join, Seat()).text("pin"))

        val resumed = entrance(join, Seat("room" to "token", 2))
        assertEquals("resume", resumed.text("type"))
        assertEquals("room", resumed.text("roomId"))
    }

    @Test
    fun aServerIsKnownByItsGreeting() {
        assertEquals(Result.success(Unit), greeting("""{"type":"hello","version":2,"at":5}"""))
        assertEquals("This relay does not support Listen Together v2.", greeting("""{"type":"hello","version":1}""").exceptionOrNull()?.message)
        assertEquals("Go away.", greeting("""{"type":"error","message":"Go away."}""").exceptionOrNull()?.message)
        assertEquals("The server returned an invalid response.", greeting("<html>").exceptionOrNull()?.message)
    }

    @Test
    fun noTwoCommandsShareAName() {
        assertNotEquals(operationId(), operationId())
    }

    private fun seek(to: Long) = JsonObject(mapOf("kind" to JsonPrimitive("seek"), "positionMs" to JsonPrimitive(to)))

    @Test
    fun aScrubSendsOnlyWhereItSettles() {
        val held = HeldSeek()
        val start = 1_000_000L
        val step = 40L
        // Three steps of a drag, closer together than a seek takes to settle.
        listOf(10_000L, 20_000L, 30_000L).forEachIndexed { index, to ->
            val now = start + step * index
            assertNull(held.settled(now))
            held.hold(seek(to), "e1", now)
        }
        val last = start + step * 2
        assertNull(held.settled(last + 100))
        val sent = held.settled(last + SEEK_SETTLES_MS)!!
        assertEquals(30_000L, sent["positionMs"]!!.jsonPrimitive.content.toLong())
        // Sent once.
        assertNull(held.settled(last + SEEK_SETTLES_MS * 2))
    }

    @Test
    fun aSeekIsPinnedToTheSongItWasAskedOf() {
        val held = HeldSeek()
        held.hold(seek(5_000), "e1", 0)
        // The room has moved on by the time it settles: the seek still names
        // the song it was meant for, and the relay refuses it.
        val sent = held.settled(SEEK_SETTLES_MS)!!
        val command = enveloped(sent, 7, "e2")["command"]!!.jsonObject
        assertEquals("e1", command.text("current"))
        assertEquals("7", command.text("base"))
    }
}

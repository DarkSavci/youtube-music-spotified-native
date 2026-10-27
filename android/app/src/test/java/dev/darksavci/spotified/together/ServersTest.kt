package dev.darksavci.spotified.together

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ServersTest {
    private fun form(name: String, url: String) = ServerForm(name = name, url = url)

    @Test
    fun aServerSavedWithoutANameIsCalledByItsHost() {
        val (servers, id) = save(emptyList(), form(" ", "wss://listen.example.com/rooms")).getOrThrow()
        assertEquals("server-1", id)
        assertEquals("listen.example.com", servers[0].name)
        val (more, second) = save(servers, form("Ours", "ws://localhost:8766")).getOrThrow()
        assertEquals("server-2", second)
        assertEquals("Ours", more[1].name)
    }

    @Test
    fun anAddressNoRoomCouldBeReachedAtIsNotSaved() {
        assertTrue(save(emptyList(), form("Ours", "http://example.com")).isFailure)
    }

    @Test
    fun editingAServerReplacesItUnderTheSameId() {
        val (servers, _) = save(emptyList(), form("Ours", "ws://localhost:8766")).getOrThrow()
        val edit = ServerForm.on(servers.first()).copy(name = "Renamed")
        val (edited, id) = save(servers, edit).getOrThrow()
        assertEquals("server-1", id)
        assertEquals(1, edited.size)
        assertEquals("Renamed", edited[0].name)
    }

    @Test
    fun aPinIsFoundInWhatWasTypedOrPasted() {
        assertEquals("01234567", pinFrom("0123 4567"))
        assertEquals("01234567", pinFrom("https://listen.example.com/join?pin=01234567&x=9"))
        assertEquals("0123", pinFrom("0123"))
        assertEquals("12345678", pinFrom("123456789"))
    }
}

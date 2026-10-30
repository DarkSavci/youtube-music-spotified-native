package dev.darksavci.spotified.together

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class InviteTest {
    private val server = "wss://listen1.example.com/rooms"

    @Test
    fun aLinkReadsBackAsTheRoomItWasMadeFor() {
        val link = inviteLink(server, "01234567")!!
        assertEquals(Invite("01234567", server), readInvite(link).getOrThrow())
    }

    @Test
    fun aLinkIsFoundInTheTextThatIsShared() {
        val text = inviteText("Friday", server, "01234567")
        assertEquals(Invite("01234567", server), readInvite(text).getOrThrow())
        assertTrue(text.contains("0123 4567"))
        assertTrue(text.contains(server))
    }

    @Test
    fun theDesktopsPinFieldFindsThePinInALink() {
        // It keeps the first eight digits of what is pasted, and this server's address has digits too.
        val digits = inviteLink(server, "01234567")!!.filter(Char::isDigit).take(8)
        assertEquals("01234567", digits)
        assertEquals("01234567", pinFrom(inviteLink(server, "01234567")!!))
    }

    @Test
    fun punctuationAfterALinkIsNotPartOfIt() {
        val link = inviteLink(server, "01234567")!!
        assertEquals(server, readInvite("Come along: $link.").getOrThrow().server)
    }

    @Test
    fun aLinkMissingAPartIsRefused() {
        assertTrue(readInvite("spotified://join?pin=01234567").isFailure)
        assertTrue(readInvite("spotified://join?server=wss%3A%2F%2Fa.example.com").isFailure)
        assertTrue(readInvite("spotified://join?pin=123&server=wss%3A%2F%2Fa.example.com").isFailure)
        assertTrue(readInvite("spotified://join?pin=01234567&server=%zz").isFailure)
        assertTrue(readInvite("nothing here").isFailure)
    }

    @Test
    fun aLinkCannotPointAtThisDeviceOrAnInsecureServer() {
        assertTrue(readInvite("spotified://join?pin=01234567&server=ws%3A%2F%2Flocalhost%3A8766").isFailure)
        assertTrue(readInvite("spotified://join?pin=01234567&server=ws%3A%2F%2Fevil.example.com").isFailure)
        assertTrue(readInvite("spotified://join?pin=01234567&server=https%3A%2F%2Fevil.example.com").isFailure)
        assertTrue(readInvite("spotified://join?pin=01234567&server=wss%3A%2F%2Fme%40evil.example.com").isFailure)
        assertTrue(readInvite("spotified://join?pin=01234567&server=wss%3A%2F%2Fa.example.com%2Fa%20b").isFailure)
    }

    @Test
    fun aRoomOnALocalServerHasNoLinkButCanStillBeShared() {
        assertNull(inviteLink("ws://localhost:8766", "01234567"))
        val text = inviteText("Friday", "ws://localhost:8766", "01234567")
        assertFalse(text.contains("spotified://"))
        assertTrue(text.contains("0123 4567"))
    }

    @Test
    fun anInvitationFindsTheServerAlreadySaved() {
        val saved = SavedServer("server-3", "Ours", "$server/")
        val invite = Invite("01234567", server)
        assertEquals(saved, invite.savedIn(listOf(saved)))
        val (servers, chosen) = invite.withServerIn(listOf(saved))!!
        assertEquals(listOf(saved), servers)
        assertEquals("server-3", chosen.id)
    }

    @Test
    fun anInvitationToANewServerSavesItUnderItsHost() {
        val other = SavedServer("server-3", "Ours", "wss://other.example.com")
        val (servers, chosen) = Invite("01234567", server).withServerIn(listOf(other))!!
        assertEquals(2, servers.size)
        assertEquals("server-4", chosen.id)
        assertEquals("listen1.example.com", chosen.name)
        assertEquals(server, chosen.url)
    }

    @Test
    fun theQuestionNamesAnUnknownServerAndWhatItSees() {
        val invite = Invite("01234567", server)
        val strange = inviteQuestion(invite, null, null)
        assertTrue(strange.contains("listen1.example.com"))
        assertTrue(strange.contains("internet address"))
        val known = inviteQuestion(invite, "Ours", "Friday")
        assertTrue(known.contains("Ours"))
        assertFalse(known.contains("internet address"))
        assertTrue(known.contains("You’ll leave “Friday”"))
    }
}

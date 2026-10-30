package dev.darksavci.spotified.data

import dev.darksavci.spotified.api.AlbumRef
import dev.darksavci.spotified.api.ArtistRef
import dev.darksavci.spotified.api.Track
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/** The same cases as crates/app/src/blocked.rs. */
class BlockedTest {
    private val song = Track(
        id = "s",
        artists = listOf(ArtistRef("UCa", "A"), ArtistRef("UCb", "B")),
        album = AlbumRef("MPREb", "Album"),
    )

    @Test
    fun aSongIsStoppedByItselfItsAlbumOrAnyOfItsArtists() {
        for ((kind, id) in listOf(BlockKind.Song to "s", BlockKind.Album to "MPREb", BlockKind.Artist to "UCb")) {
            assertFalse(Blocked().stops(song))
            val blocked = Blocked().set(kind, id, "name", true)
            assertTrue("$kind", blocked.stops(song))
            assertFalse("$kind", blocked.set(kind, id, "name", false).stops(song))
        }
    }

    @Test
    fun anArtistFromTheLibraryIsTheSameArtist() {
        val blocked = Blocked().set(BlockKind.Artist, "MPLAUCa", "A", true)
        assertTrue(blocked.has(BlockKind.Artist, "UCa"))
        assertEquals(listOf("UCa"), blocked.coreArtists)
        // Blocked once, however it is named.
        assertSame(blocked, blocked.set(BlockKind.Artist, "UCa", "A", true))
    }

    @Test
    fun aBlockedAlbumsSongsAreStoppedWhereverTheyTurnUp() {
        // A song as a Home shelf gives it: no album named.
        val bare = song.copy(album = null)
        val blocked = Blocked().set(BlockKind.Album, "MPREb", "Album", true)
        assertFalse(blocked.stops(bare))
        assertEquals(listOf("MPREb"), blocked.albumsUnread)

        val learned = blocked.learnAlbum("MPREb", listOf("s", "t"))
        assertTrue(learned.stops(bare))
        assertTrue(learned.albumsUnread.isEmpty())
        assertEquals(listOf("s", "t"), learned.coreTracks)
        assertEquals(listOf("MPREb"), learned.coreAlbums)
        // An album that is not blocked teaches nothing.
        assertSame(learned, learned.learnAlbum("MPREother", listOf("u")))

        // Unblocked, its songs play again.
        val freed = learned.set(BlockKind.Album, "MPREb", "Album", false)
        assertFalse(freed.stops(bare))
        assertTrue(freed.coreTracks.isEmpty())
    }

    @Test
    fun nothingWithoutAnIdIsBlocked() {
        assertTrue(Blocked().set(BlockKind.Artist, "", "Somebody", true).isEmpty)
    }

    @Test
    fun theCoreIsToldWhatIsBlocked() {
        val settings = Settings(
            blocked = Blocked().set(BlockKind.Song, "s", "S", true).set(BlockKind.Artist, "UCa", "A", true).set(BlockKind.Album, "MPREb", "B", true),
        )
        val core = settings.forCore(metered = false)
        assertEquals(listOf("s"), core.blockedTracks)
        assertEquals(listOf("UCa"), core.blockedArtists)
        assertEquals(listOf("MPREb"), core.blockedAlbums)
    }
}

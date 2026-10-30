package dev.darksavci.spotified.together

import dev.darksavci.spotified.api.Track
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RulesTest {
    private fun member(id: String, role: String) = Member(id = id, name = id, role = role, connected = true)

    private fun entry(id: String, track: String, by: String) =
        Entry(id = id, track = RoomTrack(id = track, title = track.uppercase()), addedBy = Person(id = by, name = by))

    private fun request(id: String, track: String, by: String) =
        SongRequest(id = id, track = RoomTrack(id = track, title = track.uppercase()), by = Person(id = by, name = by))

    /** A room that takes requests: a leader, a DJ and a guest, three songs in the queue with the second playing. */
    private fun room() = Room(
        owner = "leader",
        mode = Mode.Contributions,
        members = listOf(member("leader", "listener"), member("dj", "dj"), member("guest", "listener")),
        queue = listOf(entry("e1", "one", "leader"), entry("e2", "two", "leader"), entry("e3", "three", "guest")),
        current = "e2",
        revision = 4,
    )

    private fun track(id: String) = Track(id = id, playable = true)

    @Test
    fun aGuestsAdditionsWaitForApprovalOnlyWhereTheLeaderApproves() {
        val room = room()
        assertTrue(room.requesting("guest"))
        assertFalse(room.requesting("dj"))
        assertFalse(room.requesting("leader"))
        assertFalse(room.copy(autoAccept = true).requesting("guest"))
        val listen = room.copy(mode = Mode.Listen)
        assertFalse(listen.requesting("guest"))
        assertFalse(listen.mayAdd("guest"))
        assertTrue(listen.mayAdd("dj"))
        val everyone = room.copy(mode = Mode.Collaborative)
        assertFalse(everyone.requesting("guest"))
        assertTrue(everyone.mayAdd("guest"))
    }

    @Test
    fun theLeaderAndDjsAnswerRequestsAndAGuestSeesOnlyTheirOwn() {
        val room = room().copy(requests = listOf(request("r1", "aaa", "guest"), request("r2", "bbb", "other")))
        assertTrue(room.answers("leader"))
        assertTrue(room.answers("dj"))
        assertFalse(room.answers("guest"))
        assertEquals(2, room.requestsFor("dj").size)
        assertEquals(listOf("r1"), room.requestsFor("guest").map { it.id })
    }

    @Test
    fun aGuestMayRemoveOnlyWhatTheyAddedAndNobodyTheCurrentSong() {
        val room = room()
        val (first, current, theirs) = room.queue
        assertTrue(room.mayRemove("guest", theirs))
        assertFalse(room.mayRemove("guest", first))
        assertTrue(room.mayRemove("leader", first))
        assertFalse(room.mayRemove("leader", current))
    }

    @Test
    fun anEditIsTakenBackByWhoMadeItOrTheLeaderAndOnlyInTime() {
        val room = room().copy(undo = Undo(revision = 4, expires = 10_000.0, by = "dj"))
        assertTrue(room.mayUndo("dj", 9000.0))
        assertTrue(room.mayUndo("leader", 9000.0))
        assertFalse(room.mayUndo("guest", 9000.0))
        assertFalse(room.mayUndo("dj", 10_001.0))
        // The room has moved on since the edit: the relay would refuse it.
        assertFalse(room.copy(revision = 5).mayUndo("dj", 9000.0))
    }

    @Test
    fun theQueueIsShownFromTheSongThatIsPlaying() {
        val room = room()
        assertEquals(listOf(1, 2), room.upcoming().map { it.index })
        assertEquals(1, room.left())
        assertEquals("e3", room.nextEntry()?.id)
        assertEquals(3, room.copy(current = null).upcoming().size)
    }

    @Test
    fun onlyTheConnectedCountTowardsAReadyCheck() {
        val room = room()
        val members = room.members.toMutableList()
        members[0] = members[0].copy(ready = true)
        members[1] = members[1].copy(ready = true, connected = false)
        assertEquals(1 to 2, room.copy(members = members).ready())
    }

    @Test
    fun theLeaderIsToldOfANewRequestAndOfSeveralInOneLine() {
        val before = room()
        val withdrawn = mutableListOf<String>()
        val one = room().copy(requests = listOf(request("r1", "aaa", "guest")))
        assertEquals(listOf("guest requested “AAA”."), requestNews(before, one, "leader", withdrawn))
        val two = one.copy(requests = one.requests + request("r2", "bbb", "guest"))
        assertEquals(listOf("2 new song requests."), requestNews(before, two, "dj", withdrawn))
    }

    @Test
    fun aGuestIsToldTheirRequestWasSentAndNotAboutAnyoneElses() {
        val before = room()
        val after = room().copy(requests = listOf(request("r1", "aaa", "guest"), request("r2", "bbb", "other")))
        val withdrawn = mutableListOf<String>()
        assertEquals(listOf("Request sent. The leader decides what plays."), requestNews(before, after, "guest", withdrawn))
        assertTrue(requestNews(after, after, "guest", withdrawn).isEmpty())
    }

    @Test
    fun aGuestIsToldWhatBecameOfTheirRequests() {
        val before = room().copy(requests = listOf(request("r1", "aaa", "guest"), request("r2", "bbb", "guest")))
        val withdrawn = mutableListOf<String>()
        // One accepted: it is in the queue, marked with the request it was.
        val accepted = entry("e4", "aaa", "guest").copy(request = "r1")
        val after = room().copy(requests = listOf(request("r2", "bbb", "guest")), queue = room().queue + accepted)
        assertEquals(listOf("“AAA” was added to the queue."), requestNews(before, after, "guest", withdrawn))
        // The other declined.
        val declined = after.copy(requests = emptyList())
        assertEquals(listOf("Your request for “BBB” wasn’t added."), requestNews(after, declined, "guest", withdrawn))
        // Both answered at once, one each way.
        assertEquals(
            listOf("1 of your requests was added to the queue, 1 wasn’t."),
            requestNews(before, declined, "guest", withdrawn),
        )
    }

    @Test
    fun aRequestTheGuestWithdrewIsNotSaidToHaveBeenRefused() {
        val before = room().copy(requests = listOf(request("r1", "aaa", "guest")))
        val withdrawn = mutableListOf("r1")
        assertTrue(requestNews(before, room(), "guest", withdrawn).isEmpty())
        assertTrue(withdrawn.isEmpty())
    }

    @Test
    fun radioLeavesOutWhatTheRoomHasAndHasHadAndWhatCannotPlay() {
        val room = room().copy(history = listOf(entry("h1", "old", "leader")))
        val found = listOf(
            track("two"), track("old"), track("heard"), track("seed"),
            track("gone").copy(playable = false), track("new"), track("new"),
        )
        val fresh = freshRadio(found, "seed", room, listOf("heard"))
        assertEquals(listOf("new"), fresh.map { it.id })
    }

    @Test
    fun onlySoMuchRadioWaitsInARoomAtOnce() {
        val room = room()
        assertEquals(50, room.radioAllowance())
        val waiting = (0 until 48).map { entry("r$it", "radio$it", "leader").copy(radio = true) }
        val full = room.copy(queue = room.queue + waiting)
        assertEquals(2, full.radioAllowance())
        val found = (0 until 10).map { track("x$it") }
        assertEquals(2, freshRadio(found, "seed", full, emptyList()).size)
    }

    @Test
    fun whatTheRoomHasPlayedIsRememberedMostRecentLast() {
        val room = room().copy(history = listOf(entry("h1", "old", "leader")))
        val heard = mutableListOf("one", "before")
        rememberHeard(heard, room)
        assertEquals(listOf("before", "old", "one", "two"), heard)
    }

    @Test
    fun aSeekInTheRoomIsToldFromTheSongRunningOn() {
        val before = room().copy(playing = true, positionMs = 10_000.0, at = 1000.0)
        val after = before.copy(positionMs = 12_000.0, at = 3000.0)
        assertFalse(jumped(before, after))
        val sought = after.copy(positionMs = 60_000.0)
        assertTrue(jumped(before, sought))
        // Another song is not a seek.
        assertFalse(jumped(before, sought.copy(current = "e3")))
    }

    @Test
    fun theTimeLeftToUndoCountsUpToWholeSeconds() {
        val room = room().copy(undo = Undo(revision = 4, expires = 10_000.0, by = "guest"))
        assertEquals(10, room.undoSecondsLeft(0.0))
        assertEquals(1, room.undoSecondsLeft(9_999.0))
        assertEquals(0, room.undoSecondsLeft(10_000.0))
        assertEquals(0, room.undoSecondsLeft(20_000.0))
        assertEquals(0, room().undoSecondsLeft(0.0))
    }

    @Test
    fun aRemovalIsToldOnceTheRoomShowsIt() {
        val edit = PendingEdit.Removal("e3", "Three")
        assertEquals(null, edit.outcomeIn(room()))
        assertEquals("Removed “Three”", edit.outcomeIn(room().copy(queue = room().queue.dropLast(1))))
    }

    @Test
    fun anAcceptanceIsToldOnceNoneOfItsRequestsWait() {
        val edit = PendingEdit.Acceptance(listOf("r1", "r2"))
        val waiting = room().copy(requests = listOf(request("r1", "aaa", "guest")))
        assertEquals(null, edit.outcomeIn(waiting))
        val one = room().queue + entry("e4", "aaa", "guest").copy(request = "r1")
        assertEquals("Added “AAA” to the queue", edit.outcomeIn(room().copy(queue = one)))
        val two = one + entry("e5", "bbb", "guest").copy(request = "r2")
        assertEquals("Added 2 songs to the queue", edit.outcomeIn(room().copy(queue = two)))
        // The guest took them back first: nothing was added, and nothing is said.
        assertEquals("", edit.outcomeIn(room()))
    }

    @Test
    fun aSongShowsThePictureOfWhoAddedIt() {
        val theirs = "https://lh3.googleusercontent.com/a"
        val now = "https://lh3.googleusercontent.com/b"
        val withPicture = entry("e9", "nine", "guest").let { it.copy(addedBy = it.addedBy.copy(avatar = theirs)) }
        val room = room().copy(members = listOf(member("guest", "listener").copy(avatar = now)))
        assertEquals(theirs, room.pictureOfAdder(withPicture).single().url)
        assertEquals(now, room.pictureOfAdder(entry("e9", "nine", "guest")).single().url)
        assertTrue(room().pictureOfAdder(entry("e9", "nine", "gone")).isEmpty())
    }
}

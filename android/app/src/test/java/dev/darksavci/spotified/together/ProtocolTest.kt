package dev.darksavci.spotified.together

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ProtocolTest {
    private fun state(text: String): Room = (Incoming.read(text) as Incoming.State).room

    @Test
    fun aStateIsReadWithItsQueueAndIgnoresWhatItDoesNotKnow() {
        val room = state(
            """{"type":"state","room":{
            "name":"Friday","id":"r","pin":"01234567","owner":"m1","mode":"listen",
            "members":[{"id":"m1","name":"Ada","connected":true,"status":"listening","role":"listener","statusEntry":null}],
            "queue":[{"id":"e1","track":{"id":"abcdefghijk","title":"Song","artists":[{"name":"Band"}],"durationMs":200000,"artwork":[]},
                      "addedBy":{"id":"m1","name":"Ada","avatar":""},"addedAt":1,"catalogueMs":200000}],
            "current":"e1","positionMs":5000,"at":1000,"playing":true,"revision":7,"expires":99,"somethingNew":true}}""",
        )
        assertEquals(Mode.Listen, room.mode)
        assertEquals(7L, room.revision)
        assertEquals(0, room.currentEntry()?.index)
        assertEquals("Band", room.tracks()[0].artistNames)
        assertEquals("Ada", room.leader()?.name)
    }

    @Test
    fun thePositionRunsOnWhilePlayingAndStopsAtTheSongsEnd() {
        val room = Room(
            queue = listOf(Entry(id = "e1", track = RoomTrack(durationMs = 10_000.0))),
            current = "e1",
            positionMs = 2000.0,
            at = 1000.0,
            playing = true,
        )
        assertEquals(5000L, room.positionAt(4000.0))
        assertEquals(10_000L, room.positionAt(60_000.0))
        assertEquals(2000L, room.copy(playing = false).positionAt(4000.0))
    }

    @Test
    fun aRoomsLastDisplayChangeIsReadWhenThereIsOne() {
        val room = RelayJson.decodeFromString(Room.serializer(), """{"revision":9,"video":{"shown":true,"by":"m2","revision":8,"more":1}}""")
        assertEquals(RoomVideo(shown = true, by = "m2", revision = 8), room.video)
        val quiet = RelayJson.decodeFromString(Room.serializer(), """{"revision":9,"video":null}""")
        assertNull(quiet.video)
        assertNull(Room().video)
    }

    @Test
    fun whoMaySteerFollowsTheRoomsMode() {
        val room = Room(owner = "leader", mode = Mode.Listen, members = listOf(Member(id = "dj", role = "dj")))
        assertTrue(room.mayControl("leader"))
        assertTrue(room.mayControl("dj"))
        assertFalse(room.mayControl("guest"))
        assertTrue(room.copy(mode = Mode.Collaborative).mayControl("guest"))
    }

    @Test
    fun messagesOfAKindThisAppDoesNotKnowArePassedOver() {
        assertEquals(Incoming.Unknown, Incoming.read("""{"type":"fireworks","loud":true}"""))
        assertEquals(Incoming.Ack, Incoming.read("""{"type":"ack","op":"x","revision":3}"""))
    }

    @Test
    fun aRemoteServerMustBeSecureAndALocalOneNeedNotBe() {
        assertTrue(checkedAddress("wss://listen.example.com/rooms").isSuccess)
        assertTrue(checkedAddress(" ws://localhost:8766 ").isSuccess)
        assertTrue(checkedAddress("ws://listen.example.com").isFailure)
        assertTrue(checkedAddress("https://listen.example.com").isFailure)
        assertTrue(checkedAddress("wss://user:pw@example.com").isFailure)
        assertTrue(checkedAddress("listen.example.com").isFailure)
    }

    /** A state as the relay sends one from a room that takes requests, with a ready check on, a request waiting and an edit that can be taken back. */
    private val busy = """{"type":"state","room":{
        "id":"r","pin":"01234567","name":"","owner":"m1","mode":"contributions","locked":true,
        "members":[
          {"id":"m1","name":"Ada","avatar":"https://yt3.ggpht.com/ada=s88","connected":true,"disconnectedAt":null,"status":"listening","role":"listener","ready":true},
          {"id":"m2","name":"Bob","avatar":"","connected":false,"disconnectedAt":5,"status":"reconnecting","role":"dj","statusEntry":"e1"}],
        "queue":[{"id":"e1","track":{"id":"abcdefghijk","title":"Song","artists":[{"name":"Band"},{"name":"Guest"}],"durationMs":200000,"artwork":[],"explicit":false,"playable":true,"isVideo":false},
                  "catalogueMs":200000,"addedBy":{"id":"m2","name":"Bob","avatar":""},"addedAt":1700000000000,"radio":true,"request":"q0"}],
        "current":"e1","positionMs":0,"at":1,"playing":false,"revision":9,"expires":1700021600000,
        "activity":[{"id":"a1","text":"Ada started the room.","at":1700000000000}],
        "history":[{"id":"e0","track":{"id":"lmnopqrstuv","title":"Before","artists":[],"durationMs":1000,"artwork":[]},"addedBy":{"id":"m1","name":"Ada","avatar":""},"addedAt":1}],
        "repeat":"all","policy":"turns","duplicates":false,"limit":3,"lastControlledBy":{"id":"m1","name":"Ada"},
        "video":null,"votes":["m2"],"voteSkip":true,"undo":{"revision":9,"expires":1700000010000,"by":"m2"},
        "joinApproval":true,"pending":[{"id":"p1","name":"Cy","avatar":"","at":3}],
        "countdown":{"expires":1700000060000,"startAt":null},
        "requests":[{"id":"q1","track":{"id":"zyxwvutsrqp","title":"Wanted","artists":[{"name":"Band"}],"durationMs":0,"artwork":[]},"by":{"id":"m2","name":"Bob","avatar":""},"at":4}],
        "autoAccept":false,"finished":false,"unplayableSince":null}}"""

    @Test
    fun aBusyRoomIsReadWithEverythingThePageShows() {
        val room = state(busy)
        assertEquals(Mode.Contributions, room.mode)
        assertEquals(Repeat.All to Policy.Turns, room.repeat to room.policy)
        assertTrue(room.locked && room.joinApproval && room.voteSkip)
        assertFalse(room.duplicates || room.autoAccept)
        assertEquals(3, room.limit)
        assertEquals(listOf("m2"), room.votes)
        assertEquals("Ada’s room", room.title())
        // Members: a picture where one is shared, and who is ready.
        assertEquals("https://yt3.ggpht.com/ada=s88", room.members[0].picture[0].url)
        assertTrue(room.members[0].ready && !room.members[1].ready)
        assertTrue(room.members[1].picture.isEmpty())
        // The queue: who added it, that it is radio, and the request it was.
        val entry = room.queue[0]
        assertEquals("m2", entry.addedBy.id)
        assertTrue(entry.radio)
        assertEquals("q0", entry.request)
        assertEquals("Band, Guest", entry.track.artistNames)
        assertEquals("Before", room.history[0].track.title)
        assertEquals("Ada started the room.", room.activity[0].text)
        assertEquals("Cy", room.pending[0].name)
        assertEquals("Bob", room.requests[0].by.name)
        assertEquals("zyxwvutsrqp", room.requests[0].track.id)
        assertEquals(9L to "m2", room.undo?.let { it.revision to it.by })
        assertTrue(room.countdown != null && room.countdown?.startAt == null)
        assertEquals("Ada", room.lastControlledBy?.name)
    }

    @Test
    fun aNewRoomIsReadWithNothingGoingOnInIt() {
        val room = state(
            """{"type":"state","room":{"id":"r","pin":"01234567","name":"Friday","members":[],
            "owner":"","mode":"collaborative","locked":false,"queue":[],"current":null,"positionMs":0,
            "at":1,"playing":false,"revision":0,"expires":2,"activity":[],"history":[],"repeat":"off",
            "policy":"fifo","duplicates":true,"limit":50,"lastControlledBy":null,"video":null,
            "votes":[],"voteSkip":false,"undo":null,"joinApproval":false,"pending":[],
            "countdown":null,"requests":[],"autoAccept":false,"finished":false,"unplayableSince":null}}""",
        )
        assertEquals("Friday", room.title())
        assertEquals(Repeat.Off to Policy.Fifo, room.repeat to room.policy)
        assertTrue(room.undo == null && room.countdown == null)
        assertNull(room.lastControlledBy)
        assertEquals(50, room.limit)
    }

    @Test
    fun aStartedCountSaysWhenTheRoomBegins() {
        val countdown = RelayJson.decodeFromString(Countdown.serializer(), """{"expires":1700000060000,"startAt":1700000003000}""")
        assertEquals(1_700_000_003_000.0, countdown.startAt!!, 0.0)
    }

    @Test
    fun repeatStepsAsThePlayersButtonDoes() {
        assertEquals(Repeat.All, Repeat.Off.next())
        assertEquals(Repeat.One, Repeat.All.next())
        assertEquals(Repeat.Off, Repeat.One.next())
        assertEquals("one", Repeat.One.wire)
    }

    @Test
    fun theRelaysOtherMessagesAreReadAsTheySay() {
        assertEquals(Incoming.Hello(2), Incoming.read("""{"type":"hello","version":2,"at":5}"""))
        assertEquals(Incoming.Joined("m1", "t", "r"), Incoming.read("""{"type":"joined","member":"m1","token":"t","roomId":"r"}"""))
        assertEquals(Incoming.Error("No.", "op1", true), Incoming.read("""{"type":"error","message":"No.","op":"op1","fatal":true}"""))
        assertEquals(Incoming.Ended(""), Incoming.read("""{"type":"ended"}"""))
        assertNull(Incoming.read("<html>"))
    }
}

package dev.darksavci.spotified.together

import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.api.Repeat as CoreRepeat
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class SyncTest {
    private fun room(): Room {
        fun entry(id: String, track: String) = Entry(id = id, track = RoomTrack(id = track, durationMs = 200_000.0))
        return Room(
            owner = "leader",
            members = listOf(Member(id = "leader")),
            queue = listOf(entry("e1", "one"), entry("e2", "two"), entry("e3", "three")),
            current = "e2",
            positionMs = 30_000.0,
            at = 1000.0,
            playing = true,
        )
    }

    private fun withTheRoom(positionMs: Long) = Local(
        following = true,
        queue = listOf("one", "two", "three"),
        current = "two",
        positionMs = positionMs,
        playing = true,
        settled = true,
    )

    private fun fields(vararg pairs: Pair<String, Any?>) = JsonObject(
        pairs.associate { (name, value) ->
            name to when (value) {
                null -> JsonNull
                is String -> JsonPrimitive(value)
                is Boolean -> JsonPrimitive(value)
                is Number -> JsonPrimitive(value)
                else -> error("not a JSON value")
            }
        },
    )

    private fun toRoom(routed: Routed, kind: String): JsonObject {
        if (routed !is Routed.ToRoom || routed.kind != kind) fail("expected $kind, got $routed")
        return (routed as Routed.ToRoom).fields
    }

    @Test
    fun aPlayerWithTheRoomIsLeftAlone() {
        assertNull(follow(room(), 1000.0, withTheRoom(30_400), "e2", null))
    }

    @Test
    fun aNewSongInTheRoomIsFollowedFromWhereTheRoomIs() {
        val command = follow(room(), 3000.0, withTheRoom(30_000), "e1", null)
        assertEquals(Command.FollowRoom(room().tracks(), 1, "e2", 32_000, true), command)
    }

    @Test
    fun aPlayerAdriftIsBroughtBackButNotTwiceInARow() {
        val local = withTheRoom(40_000)
        assertNotNull(follow(room(), 1000.0, local, "e2", null))
        assertNull(follow(room(), 1000.0, local, "e2", 1000))
        // Still loading: where it says it is cannot be trusted yet.
        val loading = withTheRoom(0).copy(settled = false)
        assertNull(follow(room(), 1000.0, loading, "e2", null))
    }

    @Test
    fun aPauseInTheRoomPausesThePlayer() {
        val command = follow(room().copy(playing = false), 9000.0, withTheRoom(30_000), "e2", null)
        assertEquals(false, command?.playing)
        assertEquals(30_000L, command?.positionMs)
    }

    @Test
    fun anEmptyRoomTakesThePlayerOnce() {
        val empty = Room()
        val outside = withTheRoom(0).copy(following = false)
        assertNotNull(follow(empty, 0.0, outside, null, null))
        assertNull(follow(empty, 0.0, withTheRoom(0), null, null))
    }

    @Test
    fun thePlayersButtonsBecomeTheRoomsCommands() {
        val room = room()
        assertEquals(Routed.ToRoom("pause", fields()), route(Command.Toggle, room, "leader", 0))
        assertEquals(Routed.ToRoom("previous", fields("restart" to true)), route(Command.Previous, room, "leader", 5000))
        assertEquals(Routed.ToRoom("jump", fields("entry" to "e3")), route(Command.Jump(2), room, "leader", 0))
        assertEquals(Routed.Core, route(Command.SetVolume(0.5f), room, "leader", 0))
    }

    @Test
    fun aMoveNamesTheEntryThatWillFollow() {
        val room = room()
        // The first to the end: nothing follows it.
        assertEquals(
            Routed.ToRoom("move", fields("entry" to "e1", "before" to null)),
            route(Command.Move(0, 2), room, "leader", 0),
        )
        // The last to the front: the first follows it.
        assertEquals(
            Routed.ToRoom("move", fields("entry" to "e3", "before" to "e1")),
            route(Command.Move(2, 0), room, "leader", 0),
        )
    }

    @Test
    fun aListenerMayOnlyDoWhatTheRoomAllows() {
        val listen = room().copy(mode = Mode.Listen)
        assertEquals(Routed.Refused(LEADER_PLAYS), route(Command.Next, listen, "guest", 0))
        val add = Command.Enqueue(emptyList(), null)
        assertEquals(Routed.Refused(LISTEN_ONLY), route(add, listen, "guest", 0))
        assertEquals(Routed.Refused(LISTEN_ONLY), route(Command.Remove(2), listen, "guest", 0))
        val open = room().copy(mode = Mode.Contributions, autoAccept = true)
        toRoom(route(add, open, "guest", 0), "enqueue")
        assertEquals(Routed.Refused(LEADER_ORDERS), route(Command.Move(0, 1), open, "guest", 0))
    }

    private fun song(id: String) = Track(id = id)

    @Test
    fun whereTheLeaderApprovesAGuestsSongGoesAsARequest() {
        val room = room().copy(mode = Mode.Contributions)
        val add = Command.Enqueue(listOf(song("abcdefghijk")), 2)
        val fields = toRoom(route(add, room, "guest", 0), "request")
        assertEquals("abcdefghijk", fields["tracks"]!!.jsonArray[0].jsonObject["id"]!!.jsonPrimitive.content)
        // A request has no place: the leader chooses where it goes.
        assertNull(fields["before"])
        // Pressing play on a song asks for that one song.
        val play = Command.Play(listOf(song("aaaaaaaaaaa"), song("bbbbbbbbbbb")), 1, "")
        val asked = toRoom(route(play, room, "guest", 0), "request")
        assertEquals(1, asked["tracks"]!!.jsonArray.size)
        assertEquals("bbbbbbbbbbb", asked["tracks"]!!.jsonArray[0].jsonObject["id"]!!.jsonPrimitive.content)
    }

    @Test
    fun aGuestMayNotPutASongAheadOfOthersButMayAddToTheEnd() {
        val room = room().copy(mode = Mode.Contributions, autoAccept = true)
        val next = Command.Enqueue(listOf(song("abcdefghijk")), 2)
        assertEquals(Routed.Refused(NOT_AHEAD), route(next, room, "guest", 0))
        // With nothing after the current song, "next" is the end.
        val last = Command.Enqueue(listOf(song("abcdefghijk")), 3)
        assertEquals(JsonNull, toRoom(route(last, room.copy(current = "e3"), "guest", 0), "enqueue")["before"])
        // The leader's "next" names the entry it goes before.
        assertEquals(JsonPrimitive("e3"), toRoom(route(next, room, "leader", 0), "enqueue")["before"])
    }

    @Test
    fun aGuestRemovesOnlyTheirOwnUpcomingSongs() {
        val base = room()
        val queue = base.queue.toMutableList()
        queue[2] = queue[2].copy(addedBy = Person(id = "guest"))
        val room = base.copy(mode = Mode.Contributions, queue = queue)
        assertEquals(Routed.ToRoom("remove", fields("entry" to "e3")), route(Command.Remove(2), room, "guest", 0))
        assertEquals(Routed.Refused(OWN_ONLY), route(Command.Remove(0), room, "guest", 0))
    }

    @Test
    fun repeatIsTheRoomsAndTheLeadersToChangeAndShuffleIsItsOrder() {
        val room = room()
        assertEquals(Routed.ToRoom("settings", fields("repeat" to "all")), route(Command.SetRepeat(CoreRepeat.All), room, "leader", 0))
        val once = room.copy(repeat = Repeat.All)
        assertEquals(Routed.ToRoom("settings", fields("repeat" to "one")), route(Command.SetRepeat(CoreRepeat.One), once, "leader", 0))
        assertEquals(Routed.Refused(LEADER_SETS), route(Command.SetRepeat(CoreRepeat.All), room, "guest", 0))
        assertEquals(Routed.Refused(NO_SHUFFLE), route(Command.SetShuffle(true), room, "leader", 0))
    }

    @Test
    fun moreSongsThanTheRoomTakesAtOnceAreSaidToHaveBeenCut() {
        val many = List(101) { song("abcdefghijk") }
        assertTrue(trimmed(Command.Enqueue(many, null)))
        assertEquals(false, trimmed(Command.Play(many, 1, "")))
        assertEquals(false, trimmed(Command.Next))
    }

    @Test
    fun howAPlayerStandsWithTheRoomIsSaidInTheOldWords() {
        val room = room()
        val standing = { local: Local, ended: Boolean -> Standing.of(room, 1000.0, local, ended) }
        assertEquals(Standing.InSync, standing(withTheRoom(30_200), false))
        assertEquals(Standing.CatchingUp, standing(withTheRoom(50_000), false))
        assertEquals(Standing.Buffering, standing(withTheRoom(0).copy(settled = false), false))
        assertEquals(Standing.WaitingForNext, standing(withTheRoom(30_000), true))
        assertEquals(Standing.Paused, Standing.of(room.copy(playing = false), 1000.0, withTheRoom(30_000), false))
        assertEquals("catching up", Standing.CatchingUp.wire(true))
        assertEquals("listening", Standing.InSync.wire(true))
        assertEquals("paused", Standing.Paused.wire(false))
        assertEquals("Paused together", Standing.Paused.label)
    }

    @Test
    fun aMeasuredLengthIsToldOnlyWhenItMends() {
        assertTrue(lengthWorthTelling(0, 200_000))
        assertTrue(lengthWorthTelling(200_000, 202_000))
        assertEquals(false, lengthWorthTelling(200_000, 200_500))
        assertEquals(false, lengthWorthTelling(200_000, 260_000))
        assertEquals(false, lengthWorthTelling(200_000, 0))
    }
}

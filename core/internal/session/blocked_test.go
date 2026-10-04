package session

import (
	"testing"

	"spotifier/internal/domain"
)

func TestBlockedTrackIsSteppedOver(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"b"}})
	playN(t, c, 3)

	// The engine is never told to have the blocked track ready.
	if got := c.Target().PreloadVideoID; got != "c" {
		t.Fatalf("preload = %q, want c", got)
	}
	c.Apply(Command{Kind: CmdNext})
	if got := currentID(c); got != "c" {
		t.Fatalf("after next: current = %q, want c", got)
	}
	// Further on, so that previous goes back rather than starting over.
	c.Apply(Command{Kind: CmdPrev})
	if got := currentID(c); got != "a" {
		t.Fatalf("after prev: current = %q, want a", got)
	}
	// It is still in the queue, to be seen and unblocked.
	if got := len(c.State().Queue.Items); got != 3 {
		t.Fatalf("queue has %d tracks, want 3", got)
	}
}

func TestBlockedArtistIsSteppedOver(t *testing.T) {
	c, _ := newCore(t)
	// Blocked by the library's wrapped id, met in the queue by the channel's.
	c.SetBlocked(Blocked{Artists: []string{"MPLAUCx"}})
	queue := tracks(3)
	queue[1].Artists = []domain.ArtistRef{{ID: "UCy", Name: "Y"}, {ID: "UCx", Name: "X"}}
	c.Apply(Command{Kind: CmdPlay, Tracks: queue})

	c.Apply(Command{Kind: CmdNext})
	if got := currentID(c); got != "c" {
		t.Fatalf("current = %q, want c", got)
	}
}

func TestBlockedAlbumIsSteppedOver(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Albums: []string{"MPREb1"}})
	queue := tracks(3)
	queue[1].Album = &domain.AlbumRef{ID: "MPREb1", Name: "One"}
	queue[2].Album = &domain.AlbumRef{ID: "MPREb2", Name: "Two"}
	c.Apply(Command{Kind: CmdPlay, Tracks: queue})

	c.Apply(Command{Kind: CmdNext})
	if got := currentID(c); got != "c" {
		t.Fatalf("current = %q, want c", got)
	}
}

func TestQueueStartsPastABlockedTrack(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"a", "b"}})
	playN(t, c, 3)
	if got := currentID(c); got != "c" {
		t.Fatalf("current = %q, want c", got)
	}
	// Nothing before it but blocked tracks: previous starts it over.
	c.Apply(Command{Kind: CmdPrev})
	if got := currentID(c); got != "c" {
		t.Fatalf("after prev: current = %q, want c", got)
	}
}

func TestBlockedSongAskedForAlonePlays(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"a"}})
	playN(t, c, 1)
	if currentID(c) != "a" || c.State().State != domain.StatePlaying {
		t.Fatal("a blocked song played by itself should play")
	}
	// The settings sent again, as on any change of them, leave it playing.
	c.SetBlocked(Blocked{Tracks: []string{"a"}})
	if currentID(c) != "a" || c.State().State != domain.StatePlaying {
		t.Fatal("resending the blocked list stopped a song that was asked for")
	}
}

func TestBlockingWhatPlaysMovesOn(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 3)
	c.SetBlocked(Blocked{Tracks: []string{"a"}})
	if got := currentID(c); got != "b" {
		t.Fatalf("current = %q, want b", got)
	}
}

func TestBlockedTracksEndTheQueue(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"b", "c"}})
	playN(t, c, 3)
	if got := c.Target().PreloadVideoID; got != "" {
		t.Fatalf("preload = %q, want none", got)
	}
	c.Apply(Command{Kind: CmdNext})
	if c.State().State != domain.StatePaused {
		t.Fatal("a queue with only blocked tracks left should stop")
	}

	// Repeating, it comes round to the one track that may play.
	c.Apply(Command{Kind: CmdSetRepeat, Repeat: domain.RepeatAll})
	c.Apply(Command{Kind: CmdNext})
	if currentID(c) != "a" || c.State().State != domain.StatePlaying {
		t.Fatalf("current = %q, want a playing", currentID(c))
	}
}

func TestUnblockedTrackPlaysAgain(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"b"}})
	playN(t, c, 3)
	c.SetBlocked(Blocked{})
	c.Apply(Command{Kind: CmdNext})
	if got := currentID(c); got != "b" {
		t.Fatalf("current = %q, want b", got)
	}
}

func TestBlockedAlbumAskedForPlaysThrough(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"a", "b", "c"}})
	playN(t, c, 3)
	c.Apply(Command{Kind: CmdNext})
	if currentID(c) != "b" || c.State().State != domain.StatePlaying {
		t.Fatalf("current = %q, want b playing", currentID(c))
	}
}

func TestBlockingTheWholeQueueStopsIt(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 3)
	c.SetBlocked(Blocked{Tracks: []string{"a", "b", "c"}})
	if currentID(c) != "a" || c.State().State != domain.StatePaused {
		t.Fatalf("current = %q %s, want a paused", currentID(c), c.State().State)
	}
}

func TestPlayOnAPausedBlockedTrackMovesOn(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 3)
	c.Apply(Command{Kind: CmdToggle})
	// Blocked while paused, it stays where it is until play is pressed.
	c.SetBlocked(Blocked{Tracks: []string{"a"}})
	if currentID(c) != "a" || c.State().State != domain.StatePaused {
		t.Fatalf("current = %q %s, want a paused", currentID(c), c.State().State)
	}
	c.Apply(Command{Kind: CmdToggle})
	if currentID(c) != "b" || c.State().State != domain.StatePlaying {
		t.Fatalf("current = %q %s, want b playing", currentID(c), c.State().State)
	}
}

func TestPlayOnABlockedTrackWithNothingAfterItPlaysIt(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 3)
	c.Apply(Command{Kind: CmdJump, At: 2})
	c.Apply(Command{Kind: CmdToggle})
	c.SetBlocked(Blocked{Tracks: []string{"c"}})
	// Nothing after it to give way to: the press is the asking.
	c.Apply(Command{Kind: CmdToggle})
	if currentID(c) != "c" || c.State().State != domain.StatePlaying {
		t.Fatalf("current = %q %s, want c playing", currentID(c), c.State().State)
	}
	// And it is not then stopped by the settings being sent again.
	c.SetBlocked(Blocked{Tracks: []string{"c"}})
	if currentID(c) != "c" || c.State().State != domain.StatePlaying {
		t.Fatalf("current = %q %s, want c still playing", currentID(c), c.State().State)
	}
}

func TestSongAskedForKeepsPlayingOnceItsRadioArrives(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"a"}})
	playN(t, c, 1)
	// Its radio fills in behind it, and the settings are sent again.
	c.Apply(Command{Kind: CmdEnqueue, Insert: tracks(3)[1:], At: -1})
	c.SetBlocked(Blocked{Tracks: []string{"a"}})
	if currentID(c) != "a" || c.State().State != domain.StatePlaying {
		t.Fatalf("current = %q %s, want a playing", currentID(c), c.State().State)
	}
	// Paused and resumed, it is still the song that was asked for.
	c.Apply(Command{Kind: CmdToggle})
	c.Apply(Command{Kind: CmdToggle})
	if currentID(c) != "a" || c.State().State != domain.StatePlaying {
		t.Fatalf("after resume: current = %q %s, want a playing", currentID(c), c.State().State)
	}
	// Once left, it is stepped over like any other.
	c.Apply(Command{Kind: CmdNext})
	c.Apply(Command{Kind: CmdPrev})
	if got := currentID(c); got != "b" {
		t.Fatalf("after prev: current = %q, want b", got)
	}
}

func TestUnblockedAndBlockedAgainIsBlockedLikeAnyOther(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"a"}})
	playN(t, c, 1)
	c.Apply(Command{Kind: CmdEnqueue, Insert: tracks(2)[1:], At: -1})
	c.SetBlocked(Blocked{})
	c.SetBlocked(Blocked{Tracks: []string{"a"}})
	if got := currentID(c); got != "b" {
		t.Fatalf("current = %q, want b", got)
	}
}

func TestStartingOnTheLastTrackBlockedComesRound(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"c"}})
	c.Apply(Command{Kind: CmdPlay, Tracks: tracks(3), StartIndex: 2})
	if got := currentID(c); got != "a" {
		t.Fatalf("current = %q, want a", got)
	}
	c.Apply(Command{Kind: CmdJump, At: 2})
	if got := currentID(c); got != "a" {
		t.Fatalf("after jump: current = %q, want a", got)
	}
}

func TestRemovingWhatPlaysSkipsABlockedTrackBehindIt(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"b"}})
	playN(t, c, 3)
	c.Apply(Command{Kind: CmdRemove, At: 0})
	if got := currentID(c); got != "c" {
		t.Fatalf("current = %q, want c", got)
	}
}

func TestShuffleStepsOverBlockedTracks(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"b", "d"}})
	c.Apply(Command{Kind: CmdSetShuffle, Shuffle: true})
	playN(t, c, 6)
	heard := map[string]bool{currentID(c): true}
	for range 10 {
		c.Apply(Command{Kind: CmdNext})
		if c.State().State != domain.StatePlaying {
			break
		}
		heard[currentID(c)] = true
	}
	if heard["b"] || heard["d"] || len(heard) != 4 {
		t.Fatalf("heard %v, want a c e f", heard)
	}
}

func TestARoomPlaysWhatIsBlocked(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"a"}})
	c.Apply(Command{Kind: CmdFollow, Tracks: tracks(2), StartIndex: 0, ExpectedID: "e1", Playing: true})
	c.SetBlocked(Blocked{Tracks: []string{"a", "b"}})
	if currentID(c) != "a" || c.State().State != domain.StatePlaying {
		t.Fatalf("current = %q %s, want a playing", currentID(c), c.State().State)
	}
}

func TestARestoredBlockedTrackGivesWayWhenPlayIsPressed(t *testing.T) {
	h := NewHub(nil, DefaultSettings(), nil)
	h.Restore(&Snapshot{Tracks: tracks(3), Index: 1})
	h.Register("d", "Device", Capabilities{})
	h.SetBlocked(t.Context(), Blocked{Tracks: []string{"b"}})
	if cur := h.Projection().State.Queue.Current(); cur == nil || cur.ID != "b" {
		t.Fatal("the restored track should wait where it was")
	}
	_, _ = h.Command(t.Context(), "d", Command{Kind: CmdToggle})
	state := h.Projection().State
	if cur := state.Queue.Current(); cur == nil || cur.ID != "c" || state.State != domain.StatePlaying {
		t.Fatalf("current = %v %s, want c playing", cur, state.State)
	}
}

// A playlist of ten with a blocked song, a blocked artist and a blocked
// album in it, played as a listener plays one.
func blockedPlaylist(c *Core) []domain.Track {
	list := tracks(10)
	list[2].Artists = []domain.ArtistRef{{ID: "UCbad", Name: "Bad"}}
	list[5].Artists = []domain.ArtistRef{{ID: "UCok", Name: "Fine"}, {ID: "UCbad", Name: "Bad"}}
	list[6].Album = &domain.AlbumRef{ID: "MPREbad", Name: "Bad album"}
	c.SetBlocked(Blocked{
		Tracks:  []string{"a", "i"},
		Artists: []string{"UCbad"},
		Albums:  []string{"MPREbad"},
	})
	return list
}

// endTrack reports the current track as played to its end.
func endTrack(c *Core) {
	epoch := c.State().Epoch
	c.HandleEngine(EngineEvent{Kind: EvPosition, Epoch: epoch, PositionMs: 179_000})
	c.HandleEngine(EngineEvent{Kind: EvEnded, Epoch: epoch})
}

func TestAPlaylistPlaysThroughWithoutWhatIsBlocked(t *testing.T) {
	c, _ := newCore(t)
	list := blockedPlaylist(c)
	// The play button: from the top, where the first song is blocked.
	c.Apply(Command{Kind: CmdPlay, Tracks: list, Origin: "Playlist"})
	var heard string
	for c.State().State == domain.StatePlaying && len(heard) < 20 {
		heard += currentID(c)
		// What is made ready for a gapless change is what plays next.
		ready := c.Target().PreloadVideoID
		endTrack(c)
		if c.State().State == domain.StatePlaying && currentID(c) != ready {
			t.Fatalf("after %s the engine had %q ready but %q played", heard, ready, currentID(c))
		}
	}
	if heard != "bdehj" {
		t.Fatalf("heard %q, want bdehj", heard)
	}
	if got := len(c.State().Queue.Items); got != 10 {
		t.Fatalf("the queue has %d songs, want all 10 still listed", got)
	}
}

func TestAPlaylistRowThatIsBlockedStartsTheNextSong(t *testing.T) {
	c, _ := newCore(t)
	list := blockedPlaylist(c)
	// A double click on the sixth row, by the blocked artist; the seventh
	// is on the blocked album.
	c.Apply(Command{Kind: CmdPlay, Tracks: list, StartIndex: 5, Origin: "Playlist"})
	if got := currentID(c); got != "h" {
		t.Fatalf("current = %q, want h", got)
	}
	// Back over the blocked ones to the song before them.
	c.Apply(Command{Kind: CmdPrev})
	if got := currentID(c); got != "e" {
		t.Fatalf("after prev: current = %q, want e", got)
	}
}

func TestARepeatingPlaylistComesRoundPastWhatIsBlocked(t *testing.T) {
	c, _ := newCore(t)
	list := blockedPlaylist(c)
	c.Apply(Command{Kind: CmdSetRepeat, Repeat: domain.RepeatAll})
	c.Apply(Command{Kind: CmdPlay, Tracks: list, StartIndex: 9, Origin: "Playlist"})
	var heard string
	for range 6 {
		heard += currentID(c)
		endTrack(c)
	}
	if heard != "jbdehj" {
		t.Fatalf("heard %q, want jbdehj", heard)
	}
}

func TestAShuffledPlaylistLeavesOutWhatIsBlocked(t *testing.T) {
	c, _ := newCore(t)
	list := blockedPlaylist(c)
	c.Apply(Command{Kind: CmdSetShuffle, Shuffle: true})
	c.Apply(Command{Kind: CmdPlay, Tracks: list, Origin: "Playlist"})
	heard := map[string]bool{}
	for c.State().State == domain.StatePlaying && len(heard) < 20 {
		heard[currentID(c)] = true
		endTrack(c)
	}
	if len(heard) != 5 {
		t.Fatalf("heard %v, want b d e h j", heard)
	}
	for _, id := range []string{"a", "c", "f", "g", "i"} {
		if heard[id] {
			t.Fatalf("%s is blocked and was played: %v", id, heard)
		}
	}
}

func TestBlockingInAPlaylistThatIsPlaying(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 5)
	// The song after this one is blocked while this one plays: the engine
	// is given another to have ready.
	c.SetBlocked(Blocked{Tracks: []string{"b"}})
	if got := c.Target().PreloadVideoID; got != "c" {
		t.Fatalf("preload = %q, want c", got)
	}
	endTrack(c)
	if got := currentID(c); got != "c" {
		t.Fatalf("current = %q, want c", got)
	}
	// What plays is blocked: it gives way at once.
	c.SetBlocked(Blocked{Tracks: []string{"b", "c"}})
	if got := currentID(c); got != "d" {
		t.Fatalf("current = %q, want d", got)
	}
	// Unblocked, the earlier songs are there to go back to.
	c.SetBlocked(Blocked{})
	c.Apply(Command{Kind: CmdPrev})
	if got := currentID(c); got != "c" {
		t.Fatalf("after unblocking, prev went to %q, want c", got)
	}
}

func TestAFailedTrackMovesOnPastWhatIsBlocked(t *testing.T) {
	c, _ := newCore(t)
	c.SetBlocked(Blocked{Tracks: []string{"b"}})
	playN(t, c, 3)
	c.HandleEngine(EngineEvent{Kind: EvFailed, Epoch: c.State().Epoch, Reason: "unavailable"})
	if got := currentID(c); got != "c" {
		t.Fatalf("current = %q, want c", got)
	}
}

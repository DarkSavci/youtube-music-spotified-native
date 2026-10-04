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

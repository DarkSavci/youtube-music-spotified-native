package session

import (
	"testing"

	"spotifier/internal/domain"
)

// A skip is the listener choosing a track now: it cuts. Only a track ending
// on its own crossfades into the next.
func TestSkipCutsButAnEndingCrossfades(t *testing.T) {
	c, _ := newCore(t)
	c.settings.CrossfadeMs = 6000
	playN(t, c, 4)

	if !c.Target().UserChange {
		t.Error("a track the listener picked was not marked as their change")
	}
	c.Apply(Command{Kind: CmdNext})
	if !c.Target().UserChange {
		t.Error("a skip was not marked as the listener's change")
	}
	c.HandleEngine(EngineEvent{Kind: EvEnded, Epoch: c.State().Epoch})
	if c.Target().UserChange {
		t.Error("a track ending on its own was marked as a skip")
	}
	c.Apply(Command{Kind: CmdPrev})
	if !c.Target().UserChange {
		t.Error("going back was not marked as the listener's change")
	}
	// The configured transition still stands for the next natural change.
	if tr := c.Target().Transition; tr.Kind != "crossfade" {
		t.Errorf("transition became %q", tr.Kind)
	}
}

// Tracks of one album that follow each other play back to back, whatever
// the crossfade: a live album's joins are part of it. Into another album,
// or with no album known, the crossfade stands.
func TestAnAlbumPlaysOnWithoutACrossfade(t *testing.T) {
	c, _ := newCore(t)
	c.settings.CrossfadeMs = 6000
	queue := tracks(3)
	live := &domain.AlbumRef{ID: "live", Name: "Live"}
	queue[0].Album, queue[1].Album = live, live
	queue[2].Album = &domain.AlbumRef{ID: "other", Name: "Other"}
	if r, _ := c.Apply(Command{Kind: CmdPlay, Tracks: queue, Origin: "Test"}); r != RejectNone {
		t.Fatalf("play rejected: %s", r)
	}
	if tr := c.Target().Transition; tr.Kind != "gapless" {
		t.Errorf("into the album's next track: %q", tr.Kind)
	}
	c.Apply(Command{Kind: CmdNext})
	if tr := c.Target().Transition; tr.Kind != "crossfade" {
		t.Errorf("into another album: %q", tr.Kind)
	}
}

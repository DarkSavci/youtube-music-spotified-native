package session

import (
	"encoding/json"
	"strings"
	"testing"
	"time"

	"spotifier/internal/domain"
)

// A report only says where the engine is. If one changed the counter, the
// engine would take its own echo for a request to go there.
func TestReportsDoNotMoveTheSeekCounter(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 2)
	before := c.Target().SeekSeq
	epoch := c.Target().Epoch

	c.HandleEngine(EngineEvent{Kind: EvLoaded, Epoch: epoch, DurationMs: 180_000})
	c.HandleEngine(EngineEvent{Kind: EvPosition, Epoch: epoch, PositionMs: 6000, DurationMs: 180_000})
	c.HandleEngine(EngineEvent{Kind: EvStalled, Epoch: epoch})
	c.Apply(Command{Kind: CmdToggle})
	c.Apply(Command{Kind: CmdToggle})

	if got := c.Target().SeekSeq; got != before {
		t.Fatalf("seek counter moved from %d to %d without a seek", before, got)
	}
}

/*
The crossfade case: the engine runs into the next track and reports the old one
ended. The core names the new track at 0, and then publishes that 0 again when
the deck says it is loaded. Only the first may count as a request, and the deck
holds a different track from the last target when it arrives, so neither seeks.
*/
func TestTheLoadedReportAfterACrossfadeAsksForNoSeek(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 2)
	c.HandleEngine(EngineEvent{Kind: EvPosition, Epoch: c.Target().Epoch, PositionMs: 174_000})
	c.HandleEngine(EngineEvent{Kind: EvEnded, Epoch: c.Target().Epoch})

	named := c.Target()
	if named.VideoID != "b" || named.StartAtMs != 0 {
		t.Fatalf("named %q at %d, want b at 0", named.VideoID, named.StartAtMs)
	}
	c.HandleEngine(EngineEvent{Kind: EvLoaded, Epoch: named.Epoch, DurationMs: 180_000})
	if got := c.Target(); got.SeekSeq != named.SeekSeq || got.StartAtMs != 0 {
		t.Fatalf("after loaded: seq %d at %d, want seq %d at 0", got.SeekSeq, got.StartAtMs, named.SeekSeq)
	}
}

func TestEverythingThatMovesThePositionMovesTheCounter(t *testing.T) {
	cases := []struct {
		name string
		move func(*Core)
	}{
		{"a seek", func(c *Core) { c.Apply(Command{Kind: CmdSeek, PositionMs: 95_000}) }},
		{"the same song played again", func(c *Core) { playN(t, c, 3) }},
		{"a skip", func(c *Core) { c.Apply(Command{Kind: CmdNext}) }},
		{"a jump", func(c *Core) { c.Apply(Command{Kind: CmdJump, At: 2}) }},
		{"going back to the start", func(c *Core) {
			c.HandleEngine(EngineEvent{Kind: EvPosition, Epoch: c.Target().Epoch, PositionMs: 60_000})
			c.Apply(Command{Kind: CmdPrev})
		}},
		{"a transfer", func(c *Core) { c.Apply(Command{Kind: CmdTransfer, DeviceID: "other"}) }},
		{"a variant", func(c *Core) {
			c.Apply(Command{Kind: CmdVariant, ExpectedID: "a", Tracks: []domain.Track{{ID: "v", Playable: true}}})
		}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			c, _ := newCore(t)
			playN(t, c, 3)
			before := c.Target().SeekSeq
			tc.move(c)
			if got := c.Target().SeekSeq; got == before {
				t.Fatalf("seek counter stayed at %d", got)
			}
		})
	}
}

// Stopped at the end of the queue the last track is held at 0: pressing play
// has to start it over, not sit at its end.
func TestRunningOutOfQueueAsksForTheStart(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 1)
	before := c.Target().SeekSeq
	c.HandleEngine(EngineEvent{Kind: EvPosition, Epoch: c.Target().Epoch, PositionMs: 179_000})
	c.HandleEngine(EngineEvent{Kind: EvEnded, Epoch: c.Target().Epoch})
	got := c.Target()
	if got.SeekSeq == before || got.StartAtMs != 0 {
		t.Fatalf("seq %d at %d, want a new seq at 0", got.SeekSeq, got.StartAtMs)
	}
}

func TestSettingOnlineAgainStartsTheHeldTrackAtANewCount(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 2)
	c.SetOnline(false)
	c.HandleEngine(EngineEvent{Kind: EvFailed, Epoch: c.Target().Epoch, Reason: "network"})
	before := c.Target().SeekSeq
	c.SetOnline(true)
	if got := c.Target().SeekSeq; got == before {
		t.Fatalf("seek counter stayed at %d", got)
	}
}

// Each snapshot of a room is the room saying where it is, so a follower's
// correction for drift has to reach the engine as a request.
func TestARoomSnapshotOnTheSameEntryAsksForItsPosition(t *testing.T) {
	c, _ := newCore(t)
	c.Apply(Command{Kind: CmdFollow, Tracks: tracks(1), PositionMs: 50_000, Playing: true})
	before := c.Target().SeekSeq
	c.Apply(Command{Kind: CmdFollow, Tracks: tracks(1), PositionMs: 100_000, Playing: true})
	got := c.Target()
	if got.SeekSeq == before || got.StartAtMs != 100_000 {
		t.Fatalf("seq %d at %d, want a new seq at 100000", got.SeekSeq, got.StartAtMs)
	}
}

func TestLeavingARoomForTheOldQueueAsksForItsPosition(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 1)
	c.Apply(Command{Kind: CmdSeek, PositionMs: 30_000})
	c.Apply(Command{Kind: CmdFollow, Tracks: tracks(1), PositionMs: 90_000, Playing: true})
	before := c.Target().SeekSeq
	c.Apply(Command{Kind: CmdLeaveRoom})
	got := c.Target()
	if got.SeekSeq == before || got.StartAtMs != 30_000 {
		t.Fatalf("seq %d at %d, want a new seq at 30000", got.SeekSeq, got.StartAtMs)
	}
}

// Keeping the room's queue carries on from where it is: nothing to go to.
func TestLeavingARoomKeepingItsQueueAsksForNothing(t *testing.T) {
	c, _ := newCore(t)
	c.Apply(Command{Kind: CmdFollow, Tracks: tracks(1), PositionMs: 90_000, Playing: true})
	before := c.Target().SeekSeq
	c.Apply(Command{Kind: CmdLeaveRoom, KeepQueue: true})
	if got := c.Target().SeekSeq; got != before {
		t.Fatalf("seek counter moved from %d to %d", before, got)
	}
}

func TestRestoringAndHandingOverAsk(t *testing.T) {
	hub := NewHub(nil, DefaultSettings(), nil)
	before := hub.Projection().Target.SeekSeq
	hub.Restore(&Snapshot{Tracks: tracks(2), PositionMs: 42_000, SavedAt: time.Now()})
	restored := hub.Projection().Target
	if restored.SeekSeq == before || restored.StartAtMs != 42_000 {
		t.Fatalf("restored: seq %d at %d", restored.SeekSeq, restored.StartAtMs)
	}

	hub.Register("one", "one", Capabilities{})
	hub.Register("two", "two", Capabilities{})
	hub.Unregister("one")
	if got := hub.Projection().Target.SeekSeq; got == restored.SeekSeq {
		t.Fatalf("a new owner took over at the same count, %d", got)
	}
}

// A report in flight when the seek was made overwrites StartAtMs, and the
// projection that carried the seek may never have arrived. The target has to
// go on saying where the seek went.
func TestALateReportLeavesTheSeekNamed(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 2)
	c.Apply(Command{Kind: CmdSeek, PositionMs: 95_000})
	sought := c.Target()
	c.HandleEngine(EngineEvent{Kind: EvPosition, Epoch: sought.Epoch, PositionMs: 12_000, DurationMs: 180_000})

	got := c.Target()
	if got.StartAtMs != 12_000 {
		t.Fatalf("StartAtMs %d, want the echo of 12000", got.StartAtMs)
	}
	if got.SeekSeq != sought.SeekSeq || got.SeekToMs != 95_000 {
		t.Fatalf("seq %d to %d, want seq %d to 95000", got.SeekSeq, got.SeekToMs, sought.SeekSeq)
	}
}

// Each thing that moves the position says where it moved it to.
func TestEveryMoveNamesWhereItWent(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 3)
	c.Apply(Command{Kind: CmdSeek, PositionMs: 95_000})
	c.Apply(Command{Kind: CmdNext})
	if got := c.Target(); got.SeekToMs != 0 || got.StartAtMs != 0 {
		t.Fatalf("a new track: to %d at %d, want both 0", got.SeekToMs, got.StartAtMs)
	}

	c.Apply(Command{Kind: CmdSeek, PositionMs: 30_000})
	c.Apply(Command{Kind: CmdTransfer, DeviceID: "other"})
	if got := c.Target(); got.SeekToMs != 30_000 {
		t.Fatalf("a transfer went to %d, want 30000", got.SeekToMs)
	}

	hub := NewHub(nil, DefaultSettings(), nil)
	hub.Restore(&Snapshot{Tracks: tracks(2), PositionMs: 42_000, SavedAt: time.Now()})
	if got := hub.Projection().Target; got.SeekToMs != 42_000 {
		t.Fatalf("a restore went to %d, want 42000", got.SeekToMs)
	}
}

/*
Resuming an episode is three commands: its queue loaded and held paused, the seek,
then play. Nothing may sound from the start between them, so the target must not
ask to play until it has been told where.
*/
func TestPlayPausedSeekThenPlayNeverPlaysFromTheStart(t *testing.T) {
	c, _ := newCore(t)
	c.Apply(Command{Kind: CmdPlay, Tracks: tracks(2), StartIndex: 1, Paused: true})
	if got := c.Target(); got.Playing || got.VideoID != "b" {
		t.Fatalf("after play: %q playing %v, want b held", got.VideoID, got.Playing)
	}
	c.HandleEngine(EngineEvent{Kind: EvLoaded, Epoch: c.Target().Epoch, DurationMs: 180_000})
	if c.Target().Playing {
		t.Fatal("loading started it before the seek")
	}
	c.Apply(Command{Kind: CmdSeek, PositionMs: 100_000})
	if got := c.Target(); got.Playing || got.SeekToMs != 100_000 {
		t.Fatalf("after seek: playing %v to %d", got.Playing, got.SeekToMs)
	}
	c.Apply(Command{Kind: CmdToggle})
	if got := c.Target(); !got.Playing || got.StartAtMs != 100_000 {
		t.Fatalf("after toggle: playing %v at %d", got.Playing, got.StartAtMs)
	}
}

// The clients read the Go field name, as they do the rest of the Target's.
func TestTheCounterTravelsAsSeekSeq(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 1)
	raw, err := json.Marshal(c.Target())
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(raw), `"SeekSeq":`) {
		t.Fatalf("no SeekSeq in %s", raw)
	}
	if !strings.Contains(string(raw), `"SeekToMs":`) {
		t.Fatalf("no SeekToMs in %s", raw)
	}
}

package session

import (
	"encoding/json"
	"testing"

	"spotifier/internal/domain"
)

func queueIDs(c *Core) string {
	out := ""
	for _, t := range c.State().Queue.Items {
		out += t.ID
	}
	return out
}

func TestClearUpcomingLeavesWhatHasPlayedAndWhatPlays(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 5)
	c.Apply(Command{Kind: CmdJump, At: 2})
	epoch := c.State().Epoch

	if r, _ := c.Apply(Command{Kind: CmdClearUpcoming}); r != RejectNone {
		t.Fatalf("rejected: %s", r)
	}

	if got := queueIDs(c); got != "abc" {
		t.Fatalf("queue = %q, want abc", got)
	}
	if currentID(c) != "c" || c.State().Epoch != epoch || !c.Target().Playing {
		t.Fatalf("the song playing was disturbed: %q epoch %d", currentID(c), c.State().Epoch)
	}
	if got := c.Target().PreloadVideoID; got != "" {
		t.Fatalf("still preparing %q", got)
	}
}

// The reason it is one command: run as removals from a queue read earlier, a
// track ending meanwhile made the last of them remove the song playing.
func TestClearUpcomingAfterTheTrackMovedOnKeepsTheSongPlaying(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 3)
	c.HandleEngine(EngineEvent{Kind: EvPosition, Epoch: c.State().Epoch, PositionMs: 170_000})
	c.HandleEngine(EngineEvent{Kind: EvEnded, Epoch: c.State().Epoch, PositionMs: 180_000})
	if currentID(c) != "b" {
		t.Fatalf("on %q", currentID(c))
	}

	c.Apply(Command{Kind: CmdClearUpcoming})

	if got := queueIDs(c); got != "ab" || currentID(c) != "b" {
		t.Fatalf("queue %q on %q", got, currentID(c))
	}
}

func TestRestoreUpcomingPutsThemBackOnce(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 4)
	c.Apply(Command{Kind: CmdClearUpcoming})
	c.Apply(Command{Kind: CmdEnqueue, Insert: []domain.Track{{ID: "z", Playable: true}}, At: -1})

	c.Apply(Command{Kind: CmdRestoreUpcoming})
	c.Apply(Command{Kind: CmdRestoreUpcoming})

	if got := queueIDs(c); got != "abcdz" {
		t.Fatalf("queue = %q, want abcdz", got)
	}
}

func TestRestoreWithoutAClearAddsNothing(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 3)
	c.Apply(Command{Kind: CmdRestoreUpcoming})
	if got := queueIDs(c); got != "abc" {
		t.Fatalf("queue = %q", got)
	}
}

func TestANewQueueForgetsWhatWasCleared(t *testing.T) {
	c, _ := newCore(t)
	playN(t, c, 3)
	c.Apply(Command{Kind: CmdClearUpcoming})
	playN(t, c, 2)
	c.Apply(Command{Kind: CmdRestoreUpcoming})
	if got := queueIDs(c); got != "ab" {
		t.Fatalf("queue = %q", got)
	}
}

func TestClearAndRestoreKeepShuffleOffWorking(t *testing.T) {
	c, _ := newCore(t)
	c.Apply(Command{Kind: CmdSetShuffle, Shuffle: true})
	playN(t, c, 6)
	current := currentID(c)

	c.Apply(Command{Kind: CmdClearUpcoming})
	c.Apply(Command{Kind: CmdRestoreUpcoming})
	c.Apply(Command{Kind: CmdSetShuffle, Shuffle: false})

	if got := queueIDs(c); got != "abcdef" || currentID(c) != current {
		t.Fatalf("queue %q on %q, want abcdef on %q", got, currentID(c), current)
	}
}

func TestShuffleOffAfterAClearKeepsOnlyWhatIsLeft(t *testing.T) {
	c, _ := newCore(t)
	c.Apply(Command{Kind: CmdSetShuffle, Shuffle: true})
	playN(t, c, 6)
	current := currentID(c)

	c.Apply(Command{Kind: CmdClearUpcoming})
	left := len(c.State().Queue.Items)
	c.Apply(Command{Kind: CmdSetShuffle, Shuffle: false})

	if currentID(c) != current {
		t.Fatalf("on %q, want %q", currentID(c), current)
	}
	if got := len(c.State().Queue.Items); got != left {
		t.Fatalf("%d songs after shuffle off, want %d: a cleared song came back", got, left)
	}
}

func TestARoomsQueueIsNotClearedFromHere(t *testing.T) {
	c, _ := newCore(t)
	c.Apply(Command{Kind: CmdFollow, Tracks: tracks(3), ExpectedID: "e1", Playing: true})
	if r, _ := c.Apply(Command{Kind: CmdClearUpcoming}); r != RejectNotOwner {
		t.Fatalf("got %q", r)
	}
	if got := queueIDs(c); got != "abc" {
		t.Fatalf("queue = %q", got)
	}
}

func TestClearCommandsTravelByTheseNames(t *testing.T) {
	for wire, want := range map[string]CommandKind{"clear_upcoming": CmdClearUpcoming, "restore_upcoming": CmdRestoreUpcoming} {
		var got Command
		if err := json.Unmarshal([]byte(`{"Kind":"`+wire+`"}`), &got); err != nil || got.Kind != want {
			t.Fatalf("%s: %v %v", wire, got.Kind, err)
		}
	}
}

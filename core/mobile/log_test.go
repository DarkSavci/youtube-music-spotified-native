package mobile

import (
	"log/slog"
	"strings"
	"sync"
	"testing"
)

type recordedLine struct {
	priority int32
	line     string
}

type recordingSink struct {
	mu    sync.Mutex
	lines []recordedLine
}

func (s *recordingSink) Write(priority int32, line string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.lines = append(s.lines, recordedLine{priority, line})
}

// Each level reaches the app as Android's own, which is what stopped every line
// of the core showing as an error.
func TestLevelsBecomeAndroidsLevels(t *testing.T) {
	sink := &recordingSink{}
	log := slog.New(newSinkHandler(sink, slog.LevelDebug))
	log.Debug("d")
	log.Info("i")
	log.Warn("w")
	log.Error("e")
	want := []int32{logDebug, logInfo, logWarn, logError}
	if len(sink.lines) != len(want) {
		t.Fatalf("got %d lines: %v", len(sink.lines), sink.lines)
	}
	for i, p := range want {
		if sink.lines[i].priority != p {
			t.Errorf("line %d has priority %d, want %d", i, sink.lines[i].priority, p)
		}
	}
}

func TestLinesBelowTheLevelAreNotSent(t *testing.T) {
	sink := &recordingSink{}
	log := slog.New(newSinkHandler(sink, slog.LevelInfo))
	log.Debug("quiet")
	log.Info("heard")
	if len(sink.lines) != 1 || !strings.Contains(sink.lines[0].line, "heard") {
		t.Fatalf("got %v", sink.lines)
	}
}

// The Android log has the time and level already, and a line stays one line.
func TestALineCarriesItsAttributesAndNoTimeOrLevel(t *testing.T) {
	sink := &recordingSink{}
	log := slog.New(newSinkHandler(sink, slog.LevelInfo)).With("video", "abc").WithGroup("g")
	log.Info("resolve", "took", 3)
	line := sink.lines[0].line
	for _, want := range []string{"msg=resolve", "video=abc", "g.took=3"} {
		if !strings.Contains(line, want) {
			t.Errorf("no %q in %q", want, line)
		}
	}
	for _, not := range []string{"time=", "level=", "\n"} {
		if strings.Contains(line, not) {
			t.Errorf("%q in %q", not, line)
		}
	}
}

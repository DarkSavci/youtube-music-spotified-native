package run

import (
	"context"
	"encoding/json"
	"io"
	"log/slog"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"spotifier/internal/domain"
	"spotifier/internal/report"
	"spotifier/internal/session"
)

type pingRecorder struct {
	mu    sync.Mutex
	urls  []string
	asked atomic.Bool
}

func (p *pingRecorder) Call(context.Context, string, map[string]any) (json.RawMessage, error) {
	p.asked.Store(true)
	return json.RawMessage(`{
	  "playbackTracking": {
	    "videostatsPlaybackUrl": {"baseUrl": "https://s.youtube.com/api/stats/playback?docid=a"},
	    "videostatsWatchtimeUrl": {"baseUrl": "https://s.youtube.com/api/stats/watchtime?docid=a"}
	  },
	  "videoDetails": {"lengthSeconds": "180"}
	}`), nil
}

func (p *pingRecorder) GetSigned(_ context.Context, rawURL string) (int, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.urls = append(p.urls, rawURL)
	return 204, nil
}

func (p *pingRecorder) watchtime() string {
	p.mu.Lock()
	defer p.mu.Unlock()
	for _, u := range p.urls {
		if strings.Contains(u, "/watchtime") {
			return u
		}
	}
	return ""
}

// What played since the last half-minute tick is reported when the pause comes,
// long before the next tick could.
func TestAPauseReportsTheWatchTimeSinceTheLastTick(t *testing.T) {
	hub := session.NewHub(nil, session.DefaultSettings(), nil)
	pings := &pingRecorder{}
	rep := &report.Reporter{Client: pings}
	rep.SetEnabled(true)

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	go watchAndReport(ctx, hub, rep, slog.New(slog.NewTextHandler(io.Discard, nil)))

	track := domain.Track{ID: "a", Title: "A", DurationMs: 180_000, Playable: true}
	hub.Register("phone", "phone", session.Capabilities{})
	if _, err := hub.Command(ctx, "phone", session.Command{Kind: session.CmdPlay, Tracks: []domain.Track{track}}); err != nil {
		t.Fatal(err)
	}
	// Until the loop has seen the track playing, there is no pause for it to see.
	deadline := time.Now().Add(3 * time.Second)
	for !pings.asked.Load() {
		if time.Now().After(deadline) {
			t.Fatal("the track was never prepared for reporting")
		}
		time.Sleep(5 * time.Millisecond)
	}
	epoch := hub.Projection().State.Epoch
	hub.EngineEvent(ctx, "phone", session.EngineEvent{Kind: session.EvPosition, Epoch: epoch, PositionMs: 17_000})
	if _, err := hub.Command(ctx, "phone", session.Command{Kind: session.CmdToggle}); err != nil {
		t.Fatal(err)
	}

	deadline = time.Now().Add(3 * time.Second)
	for pings.watchtime() == "" {
		if time.Now().After(deadline) {
			t.Fatal("the pause sent no watch-time report")
		}
		time.Sleep(10 * time.Millisecond)
	}
	if got := pings.watchtime(); !strings.Contains(got, "cmt=17") {
		t.Fatalf("reported %s, want the position at the pause", got)
	}
}

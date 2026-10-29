package jsc_test

import (
	"context"
	"io"
	"net/http"
	"os"
	"regexp"
	"testing"
	"time"

	"spotifier/internal/jsc"
)

// Against YouTube's current player, with a real runtime:
//
//	SPOTIFIER_LIVE=1 SPOTIFIER_JS_RUNTIME=deno:../../../vendor/deno/deno.exe \
//	    go test ./internal/jsc/ -run Live -v
//
// QuickJS-NG works the same way, as quickjs:<path to qjs>.
func TestLiveSolve(t *testing.T) {
	if os.Getenv("SPOTIFIER_LIVE") != "1" {
		t.Skip("set SPOTIFIER_LIVE=1 to run live tests")
	}
	rt, err := jsc.ParseRuntime(os.Getenv("SPOTIFIER_JS_RUNTIME"))
	if err != nil {
		t.Skipf("SPOTIFIER_JS_RUNTIME: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	defer cancel()

	iframe := get(t, ctx, "https://www.youtube.com/iframe_api")
	m := regexp.MustCompile(`player\\?/([0-9a-fA-F]{8})\\?/`).FindSubmatch(iframe)
	if m == nil {
		t.Fatal("the iframe API named no player")
	}
	player := "https://www.youtube.com/s/player/" + string(m[1]) + "/player_ias.vflset/en_US/base.js"
	fetched := 0
	source := func(ctx context.Context) ([]byte, error) {
		fetched++
		return get(t, ctx, player), nil
	}

	// What yt-dlp asks: an n value as the addresses carry them, and a
	// signature of a length the player's function permutes.
	n := []string{"mT8dD3tVq1kXbAa9"}
	sig := []string{"AOq0QJ8wRQIhAKQ8n2xY7cIjb0n5o6eDbo4Rk4ZqK0q7nB0cX9fJ1Q2YAiBYm4ZpE6t0b5w3s8qLr1f2ZAeD3gH4iJ5kL6mN7oP8qR=="}

	dir := t.TempDir()
	s := jsc.New(rt, dir, nil)
	defer s.Close()

	started := time.Now()
	first, err := s.Solve(ctx, player, source, n, sig)
	if err != nil {
		t.Fatalf("first solve: %v", err)
	}
	t.Logf("first solve, parsing the player: %v", time.Since(started))
	if first.N[n[0]] == "" || first.N[n[0]] == n[0] || first.Sig[sig[0]] == "" {
		t.Errorf("unexpected answers: %+v", first)
	}

	started = time.Now()
	again, err := s.Solve(ctx, player, source, n, sig)
	if err != nil {
		t.Fatalf("second solve: %v", err)
	}
	t.Logf("second solve, player kept: %v", time.Since(started))
	if again.N[n[0]] != first.N[n[0]] || again.Sig[sig[0]] != first.Sig[sig[0]] {
		t.Error("the same challenges were answered differently")
	}

	// A new process reads the parsed player back instead of fetching it.
	s.Close()
	fresh := jsc.New(rt, dir, nil)
	defer fresh.Close()
	started = time.Now()
	third, err := fresh.Solve(ctx, player, source, n, sig)
	if err != nil {
		t.Fatalf("solve after a restart: %v", err)
	}
	t.Logf("after a restart, the parsed player from disk: %v", time.Since(started))
	if fetched != 1 {
		t.Errorf("the player was fetched %d times", fetched)
	}
	if third.N[n[0]] != first.N[n[0]] {
		t.Error("the saved player answered differently")
	}
}

func get(t *testing.T, ctx context.Context, address string) []byte {
	t.Helper()
	req, _ := http.NewRequestWithContext(ctx, http.MethodGet, address, nil)
	req.Header.Set("User-Agent", "Mozilla/5.0")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("get %s: %v", address, err)
	}
	defer resp.Body.Close()
	b, err := io.ReadAll(resp.Body)
	if err != nil || resp.StatusCode != http.StatusOK {
		t.Fatalf("get %s: %d %v", address, resp.StatusCode, err)
	}
	return b
}

package mobile

import (
	"encoding/json"
	"net/http"
	"testing"
	"time"
)

// The app starts the core, talks to it on the address it is given, and
// stops it; a second start while it runs is refused rather than racing it.
func TestStartServesAndStops(t *testing.T) {
	cfg := NewConfig()
	cfg.DataDir = t.TempDir()
	cfg.CacheDir = t.TempDir()

	addr, err := Start(cfg)
	if err != nil {
		t.Fatalf("start: %v", err)
	}
	stoppedHere := false
	defer func() {
		if !stoppedHere {
			Stop()
		}
	}()

	if _, err := Start(cfg); err == nil {
		t.Error("a second start was allowed")
	}

	client := http.Client{Timeout: 5 * time.Second}
	get := func(token string) int {
		req, _ := http.NewRequest(http.MethodGet, "http://"+addr+"/v1/health", nil)
		if token != "" {
			req.Header.Set(TokenHeader, token)
		}
		res, err := client.Do(req)
		if err != nil {
			t.Fatalf("health: %v", err)
		}
		_ = res.Body.Close()
		return res.StatusCode
	}
	// Any app on the phone can reach the address; only this one has the token.
	if code := get(""); code != http.StatusForbidden {
		t.Errorf("without the token the core answered %d", code)
	}
	if code := get("wrong"); code != http.StatusForbidden {
		t.Errorf("with a wrong token the core answered %d", code)
	}
	if code := get(Token()); code != http.StatusOK {
		t.Errorf("with the token the core answered %d", code)
	}

	Stop()
	stoppedHere = true
	if _, err := client.Get("http://" + addr + "/v1/health"); err == nil {
		t.Error("the core still answers after Stop")
	}
	if Token() != "" {
		t.Error("the token outlived the core")
	}
}

// An argument holding a newline stays one argument on the app's side.
func TestBridgeKeepsArgumentsWhole(t *testing.T) {
	var got string
	run := bridge(runnerFunc(func(args string) (string, error) {
		got = args
		return "ok", nil
	}))
	if _, _, err := run(t.Context(), []string{"-f", "best", "https://x/watch?v=a\n--exec"}); err != nil {
		t.Fatal(err)
	}
	var args []string
	if err := json.Unmarshal([]byte(got), &args); err != nil || len(args) != 3 {
		t.Errorf("want 3 arguments as JSON, got %q", got)
	}
}

type runnerFunc func(string) (string, error)

func (f runnerFunc) Run(args string) (string, error) { return f(args) }

func TestStartNeedsADataDirectory(t *testing.T) {
	if _, err := Start(NewConfig()); err == nil {
		t.Error("started without a data directory")
	}
}

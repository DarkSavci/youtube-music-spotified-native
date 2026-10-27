package resolver_test

import (
	"context"
	"net/http"
	"os"
	"testing"
	"time"

	"spotifier/internal/innertube"
	"spotifier/internal/jsc"
	"spotifier/internal/resolver"
)

// Resolves real songs with a Premium account and fetches the start of each:
//
//	SPOTIFIER_LIVE=1 SPOTIFIER_CREDENTIALS=<credentials.json> \
//	    SPOTIFIER_JS_RUNTIME=deno:<path> go test ./internal/resolver/ -run LiveNative -v
func TestLiveNative(t *testing.T) {
	if os.Getenv("SPOTIFIER_LIVE") != "1" {
		t.Skip("set SPOTIFIER_LIVE=1 to run live tests")
	}
	creds, err := innertube.LoadCredentials(os.Getenv("SPOTIFIER_CREDENTIALS"))
	if err != nil {
		t.Skipf("SPOTIFIER_CREDENTIALS: %v", err)
	}
	rt, err := jsc.ParseRuntime(os.Getenv("SPOTIFIER_JS_RUNTIME"))
	if err != nil {
		t.Skipf("SPOTIFIER_JS_RUNTIME: %v", err)
	}
	client := innertube.New(innertube.WithCredentials(creds), innertube.WithLocale("en", "US"))
	solver := jsc.New(rt, t.TempDir(), nil)
	defer solver.Close()
	n := resolver.NewNative(func() resolver.PlayerAPI { return client }, solver, t.TempDir())

	for _, id := range []string{"kffacxfA7G4", "tVj0ZTS4WF4", "JGwWNGJdvx8"} {
		ctx, cancel := context.WithTimeout(context.Background(), time.Minute)
		started := time.Now()
		st, q, err := n.Resolve(ctx, id)
		took := time.Since(started)
		if err != nil {
			cancel()
			t.Fatalf("%s: %v", id, err)
		}
		req, _ := http.NewRequestWithContext(ctx, http.MethodGet, st.URL, nil)
		req.Header.Set("Range", "bytes=0-65535")
		resp, err := http.DefaultClient.Do(req)
		cancel()
		if err != nil {
			t.Fatalf("%s: fetch: %v", id, err)
		}
		resp.Body.Close()
		t.Logf("%s: %s premium=%v in %v; the address answers %d", id, q.Label, q.Premium, took.Round(time.Millisecond), resp.StatusCode)
		if resp.StatusCode != http.StatusPartialContent && resp.StatusCode != http.StatusOK {
			t.Errorf("%s: the deciphered address was refused: %d", id, resp.StatusCode)
		}
	}
}

package resolver_test

import (
	"context"
	"errors"
	"slices"
	"testing"

	"spotifier/internal/resolver"
)

// On Android yt-dlp is Python inside the app: the resolver hands it the
// arguments it would have given the program and reads the same JSON back.
func TestYtdlpRunsInProcess(t *testing.T) {
	var asked [][]string
	y := resolver.NewYtdlp("", "")
	y.JSRuntimes = "quickjs:/lib/qjs"
	y.Run = func(_ context.Context, args []string) ([]byte, []byte, error) {
		asked = append(asked, args)
		return []byte(`{"duration": 200, "formats": [
			{"format_id": "251", "url": "https://x/251", "ext": "webm", "acodec": "opus", "vcodec": "none", "abr": 130, "protocol": "https"},
			{"format_id": "774", "url": "https://x/774", "ext": "webm", "acodec": "opus", "vcodec": "none", "abr": 256, "protocol": "https"}
		]}`), nil, nil
	}

	st, q, err := y.Resolve(context.Background(), "abc")
	if err != nil {
		t.Fatalf("resolve: %v", err)
	}
	if st.URL != "https://x/774" || !q.Premium {
		t.Errorf("want the subscriber format, got %s (premium %v)", st.URL, q.Premium)
	}
	if len(asked) != 1 {
		t.Fatalf("want one run, got %d", len(asked))
	}
	at := slices.Index(asked[0], "--js-runtimes")
	if at < 0 || asked[0][at+1] != "quickjs:/lib/qjs" {
		t.Errorf("the runtime was not passed: %v", asked[0])
	}
}

// The id comes from a request's path; one that is not YouTube's alphabet
// never reaches yt-dlp's arguments.
func TestYtdlpRefusesIDsOutsideTheAlphabet(t *testing.T) {
	y := resolver.NewYtdlp("", "")
	y.Run = func(context.Context, []string) ([]byte, []byte, error) {
		t.Error("yt-dlp was run")
		return nil, nil, nil
	}
	for _, id := range []string{"a\n--exec", "a b", "../x", ""} {
		if _, _, err := y.Resolve(context.Background(), id); !errors.Is(err, resolver.ErrUnavailable) {
			t.Errorf("%q: want ErrUnavailable, got %v", id, err)
		}
	}
}

// A failure is read from what yt-dlp said on stderr, as it is from the
// program, so a rate limit still reads as one.
func TestYtdlpInProcessFailureIsClassified(t *testing.T) {
	y := resolver.NewYtdlp("", "")
	y.Run = func(context.Context, []string) ([]byte, []byte, error) {
		return nil, []byte("ERROR: HTTP Error 429: Too Many Requests"), errors.New("exit 1")
	}
	if _, _, err := y.Resolve(context.Background(), "abc"); !errors.Is(err, resolver.ErrRateLimited) {
		t.Errorf("want ErrRateLimited, got %v", err)
	}
	// Supplied by the app, it is there by construction: probing it would
	// start Python at launch for nothing.
	if !y.Available() {
		t.Error("a supplied runner counts as available")
	}
}

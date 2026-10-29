package resolver_test

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"slices"
	"strings"
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

const oneFormat = `{"duration": 200, "formats": [
	{"format_id": "251", "url": "https://x/251", "ext": "webm", "acodec": "opus", "vcodec": "none", "abr": 130, "protocol": "https"}
]}`

// Signed out, YouTube Music's client serves nothing, so it is not asked: one
// run per track, the default way.
func TestYtdlpSignedOutAsksOnceTheDefaultWay(t *testing.T) {
	var asked []string
	y := resolver.NewYtdlp("", "")
	y.Run = func(_ context.Context, args []string) ([]byte, []byte, error) {
		asked = append(asked, args[len(args)-1])
		return []byte(oneFormat), nil, nil
	}
	if _, _, err := y.Resolve(context.Background(), "abc"); err != nil {
		t.Fatalf("resolve: %v", err)
	}
	if len(asked) != 1 || asked[0] != "https://www.youtube.com/watch?v=abc" {
		t.Errorf("want one run the default way, got %v", asked)
	}
}

// A listener YouTube Music's client refuses (signed in without Premium) has it
// tried twice; once it has failed twice running where the default worked, the
// next track goes the default way at once.
func TestYtdlpLeavesOutTheMusicClientOnceItRefused(t *testing.T) {
	cookies := filepath.Join(t.TempDir(), "cookies.txt")
	if err := os.WriteFile(cookies, []byte("# Netscape HTTP Cookie File\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	var asked []string
	y := resolver.NewYtdlp("", cookies)
	y.Run = func(_ context.Context, args []string) ([]byte, []byte, error) {
		url := args[len(args)-1]
		asked = append(asked, url)
		if strings.HasPrefix(url, "https://music.youtube.com/") {
			return nil, []byte("ERROR: [youtube] abc: Requested format is not available"), errors.New("exit 1")
		}
		return []byte(oneFormat), nil, nil
	}
	// Twice refused, as every track is for this listener; once alone would be
	// a track it cannot serve.
	for _, id := range []string{"abc", "def"} {
		asked = nil
		if _, _, err := y.Resolve(context.Background(), id); err != nil {
			t.Fatalf("%s: %v", id, err)
		}
		if len(asked) != 2 {
			t.Fatalf("%s tries YouTube Music, then the default: %v", id, asked)
		}
	}
	asked = nil
	if _, _, err := y.Resolve(context.Background(), "ghi"); err != nil {
		t.Fatalf("third: %v", err)
	}
	if len(asked) != 1 || !strings.HasPrefix(asked[0], "https://www.youtube.com/") {
		t.Errorf("the next track goes the default way only, got %v", asked)
	}
}

// A lookup called off is a cancellation, not the track's failure: it must
// stay recognisable as one, or it is remembered and the track refused.
func TestYtdlpCancelledIsACancellation(t *testing.T) {
	y := resolver.NewYtdlp("", "")
	ctx, cancel := context.WithCancel(context.Background())
	y.Run = func(context.Context, []string) ([]byte, []byte, error) {
		cancel()
		return nil, nil, errors.New("killed")
	}
	if _, _, err := y.Resolve(ctx, "abc"); !errors.Is(err, context.Canceled) {
		t.Errorf("want context.Canceled, got %v", err)
	}
}

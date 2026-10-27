package resolver

import (
	"errors"
	"strings"
	"testing"
	"unicode/utf8"
)

func TestClassify(t *testing.T) {
	cases := []struct {
		msg                  string
		rateLimited, unavail bool
	}{
		{"ERROR: [youtube] x: Sign in to confirm you're not a bot", true, false},
		{"HTTP Error 429: Too Many Requests", true, false},
		{"ERROR: [youtube] x: This content isn't available, try again later.", true, false},
		{"ERROR: [youtube] x: Sign in to confirm your age. This video may be inappropriate for some users.", false, true},
		{"ERROR: [youtube] x: Requested format is not available", false, false},
		{"ERROR: [youtube] x: Video unavailable", false, true},
		{"ERROR: [youtube] x: Private video", false, true},
	}
	for _, c := range cases {
		err := classify(errors.New(c.msg))
		if got := errors.Is(err, ErrRateLimited); got != c.rateLimited {
			t.Errorf("%q: rate limited = %v", c.msg, got)
		}
		if got := errors.Is(err, ErrUnavailable); got != c.unavail {
			t.Errorf("%q: unavailable = %v", c.msg, got)
		}
	}
}

func TestAFailureSaysWhatYtdlpPrintedOrThatItPrintedNothing(t *testing.T) {
	cases := []struct {
		stderr, stdout, want string
	}{
		{"ERROR: [youtube] abc: Video unavailable\n", "", "ERROR: [youtube] abc: Video unavailable"},
		// What it printed may hold signed stream addresses: only how much.
		{"", `{"url": "https://rr1.googlevideo.com/videoplayback?sig=x"}`, "nothing on stderr; 58 bytes on stdout"},
		{"", "", "yt-dlp printed nothing"},
	}
	for _, c := range cases {
		if got := said([]byte(c.stderr), []byte(c.stdout)); got != c.want {
			t.Errorf("said(%q, %q) = %q, want %q", c.stderr, c.stdout, got, c.want)
		}
	}
	long := strings.Repeat("ş", saidMost)
	if got := said([]byte(long), nil); !utf8.ValidString(got) || len(got) > saidMost+len("…") {
		t.Errorf("a long message was not cut cleanly: %d bytes", len(got))
	}
}

package resolver_test

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/url"
	"strings"
	"testing"

	"spotifier/internal/jsc"
	"spotifier/internal/resolver"
)

// fakePlayer answers the player request with a canned response.
type fakePlayer struct {
	signedIn bool
	answer   string
	asked    []map[string]any
}

func (f *fakePlayer) Authenticated() bool { return f.signedIn }

func (f *fakePlayer) Call(_ context.Context, endpoint string, body map[string]any) (json.RawMessage, error) {
	if endpoint != "player" {
		return nil, errors.New("unexpected endpoint " + endpoint)
	}
	f.asked = append(f.asked, body)
	return json.RawMessage(f.answer), nil
}

// fakeSolver reverses an n challenge and upper-cases a signature.
type fakeSolver struct{ keys []string }

func (f *fakeSolver) Solve(_ context.Context, key string, _ func(context.Context) ([]byte, error), n, sig []string) (jsc.Answers, error) {
	f.keys = append(f.keys, key)
	out := jsc.Answers{N: map[string]string{}, Sig: map[string]string{}}
	for _, c := range n {
		r := []rune(c)
		for i, j := 0, len(r)-1; i < j; i, j = i+1, j-1 {
			r[i], r[j] = r[j], r[i]
		}
		out.N[c] = string(r)
	}
	for _, c := range sig {
		out.Sig[c] = strings.ToUpper(c)
	}
	return out, nil
}

// youtube stands in for www.youtube.com: the iframe API and a player.
type youtube struct{}

func (youtube) RoundTrip(r *http.Request) (*http.Response, error) {
	body := ""
	switch {
	case r.URL.Path == "/iframe_api":
		body = `var scriptUrl = 'https:\/\/www.youtube.com\/s\/player\/0a1b2c3d\/www-widgetapi.vflset\/www-widgetapi.js';`
	case strings.HasSuffix(r.URL.Path, "/player_ias.vflset/en_US/base.js"):
		body = `var x={signatureTimestamp:20391,other:1};`
	default:
		return &http.Response{StatusCode: 404, Body: io.NopCloser(strings.NewReader("")), Request: r}, nil
	}
	return &http.Response{StatusCode: 200, Body: io.NopCloser(strings.NewReader(body)), Request: r}, nil
}

const premiumAnswer = `{
  "playabilityStatus": {"status": "OK"},
  "streamingData": {"adaptiveFormats": [
    {"itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 140000,
     "url": "https://rr1.googlevideo.com/videoplayback?itag=251&n=abc"},
    {"itag": 774, "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 270000, "averageBitrate": 256000,
     "contentLength": "8000000", "approxDurationMs": "240000",
     "signatureCipher": "s=sig&sp=sig&url=https%3A%2F%2Frr1.googlevideo.com%2Fvideoplayback%3Fitag%3D774%26n%3Dxyz%26expire%3D9999999999"},
    {"itag": 137, "mimeType": "video/mp4; codecs=\"avc1\"", "bitrate": 4000000, "url": "https://x/video"}
  ]}
}`

const standardAnswer = `{
  "playabilityStatus": {"status": "OK"},
  "streamingData": {"adaptiveFormats": [
    {"itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 140000,
     "url": "https://rr1.googlevideo.com/videoplayback?itag=251&n=abc"}
  ]}
}`

func newNative(t *testing.T, p *fakePlayer, s *fakeSolver) *resolver.Native {
	t.Helper()
	n := resolver.NewNative(func() resolver.PlayerAPI {
		if p == nil {
			return nil
		}
		return p
	}, s, t.TempDir())
	n.HTTP = &http.Client{Transport: youtube{}}
	return n
}

func TestNativeDeciphersThePremiumFormat(t *testing.T) {
	p := &fakePlayer{signedIn: true, answer: premiumAnswer}
	s := &fakeSolver{}
	st, q, err := newNative(t, p, s).Resolve(context.Background(), "abcdefghijk")
	if err != nil {
		t.Fatalf("resolve: %v", err)
	}
	if !q.Premium || q.Codec != "Opus" {
		t.Errorf("want the subscriber Opus format, got %+v", q)
	}
	u, err := url.Parse(st.URL)
	if err != nil {
		t.Fatal(err)
	}
	if got := u.Query().Get("sig"); got != "SIG" {
		t.Errorf("the signature should go back under sp's name: sig=%q", got)
	}
	if got := u.Query().Get("n"); got != "zyx" {
		t.Errorf("n should be replaced by its answer: n=%q", got)
	}
	if st.SizeBytes != 8_000_000 || st.DurationMs != 240_000 || st.Bitrate != 256_000 {
		t.Errorf("size, length or bitrate lost: %+v", st)
	}
	if want := "https://www.youtube.com/s/player/0a1b2c3d/player_ias.vflset/en_US/base.js"; len(s.keys) != 1 || s.keys[0] != want {
		t.Errorf("solved against %v, want %s", s.keys, want)
	}
	ctx := p.asked[0]["playbackContext"].(map[string]any)["contentPlaybackContext"].(map[string]any)
	if ctx["signatureTimestamp"] != 20391 {
		t.Errorf("the player request should carry the player's timestamp: %v", ctx)
	}
}

func TestNativeLeavesSignedOutListenersToTheFallback(t *testing.T) {
	for name, p := range map[string]*fakePlayer{"no client": nil, "not signed in": {}} {
		_, _, err := newNative(t, p, &fakeSolver{}).Resolve(context.Background(), "abcdefghijk")
		if !errors.Is(err, resolver.ErrNotApplicable) {
			t.Errorf("%s: want ErrNotApplicable, got %v", name, err)
		}
		if p != nil && len(p.asked) != 0 {
			t.Errorf("%s: a player request was made", name)
		}
	}
}

// Without subscriber formats the web client's addresses need a token this
// has not got. After a few such answers the account is left to the
// fallback without asking.
func TestNativeLeavesAccountsWithoutPremiumToTheFallback(t *testing.T) {
	p := &fakePlayer{signedIn: true, answer: standardAnswer}
	n := newNative(t, p, &fakeSolver{})
	for i := 0; i < 5; i++ {
		if _, _, err := n.Resolve(context.Background(), "abcdefghijk"); !errors.Is(err, resolver.ErrNotApplicable) {
			t.Fatalf("answer %d: want ErrNotApplicable, got %v", i, err)
		}
	}
	if len(p.asked) != 3 {
		t.Errorf("want three player requests before the verdict, got %d", len(p.asked))
	}
}

// Once Premium has shown, a track with standard formats only (a podcast,
// an upload) is still served: a Premium account needs no token for them.
func TestNativeServesStandardFormatsToAPremiumAccount(t *testing.T) {
	p := &fakePlayer{signedIn: true, answer: premiumAnswer}
	n := newNative(t, p, &fakeSolver{})
	if _, _, err := n.Resolve(context.Background(), "abcdefghijk"); err != nil {
		t.Fatal(err)
	}
	p.answer = standardAnswer
	_, q, err := n.Resolve(context.Background(), "bcdefghijkl")
	if err != nil {
		t.Fatalf("standard formats for a Premium account: %v", err)
	}
	if q.Premium {
		t.Errorf("itag 251 is not a subscriber format: %+v", q)
	}
}

func TestNativeRemovedVideoIsUnavailable(t *testing.T) {
	p := &fakePlayer{signedIn: true, answer: `{"playabilityStatus": {"status": "ERROR", "reason": "This video is unavailable"}}`}
	if _, _, err := newNative(t, p, &fakeSolver{}).Resolve(context.Background(), "abcdefghijk"); !errors.Is(err, resolver.ErrUnavailable) {
		t.Errorf("want ErrUnavailable, got %v", err)
	}
}

// On mobile data the listener may ask for less: the best under the ceiling.
func TestNativeHonoursTheQualityCeiling(t *testing.T) {
	resolver.SetQuality("standard")
	defer resolver.SetQuality("high")
	p := &fakePlayer{signedIn: true, answer: premiumAnswer}
	_, q, err := newNative(t, p, &fakeSolver{}).Resolve(context.Background(), "abcdefghijk")
	if err != nil {
		t.Fatal(err)
	}
	if q.Premium || q.Bitrate > 170_000 {
		t.Errorf("want the standard format under the ceiling, got %+v", q)
	}
}

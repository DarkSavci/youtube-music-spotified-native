package resolver

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"time"

	"spotifier/internal/domain"
	"spotifier/internal/jsc"
)

/*
The native adapter: YouTube Music's own player request, made by the core.

yt-dlp is a general tool, and resolving a song costs it a second request
(the watch page's data, for nothing the core uses), a fresh process and a
fresh parse of the player every time. This makes the one request that
matters, as YouTube Music's web client signed in with the account, and puts
the stream address through the player's sig and n functions with the
solver in internal/jsc, which keeps the player parsed. The deciphering is
still not ours (see this package's comment): it is yt-dlp-ejs, the solver
yt-dlp itself uses.

It serves Premium accounts only. YouTube Music's web client asks every other
listener for a proof-of-origin token, which neither this nor yt-dlp has;
yt-dlp reaches them through other clients, so it stays behind this one in
the chain, and anything this cannot serve is ErrNotApplicable.
*/

// PlayerAPI is the InnerTube client the native adapter calls through: the
// web client, with the account's cookies and request signature.
type PlayerAPI interface {
	Call(ctx context.Context, endpoint string, body map[string]any) (json.RawMessage, error)
	Authenticated() bool
}

// Solver answers the player's challenges; jsc.Solver is the one.
type Solver interface {
	Solve(ctx context.Context, key string, source func(context.Context) ([]byte, error), n, sig []string) (jsc.Answers, error)
}

// Native resolves with YouTube Music's player request and the solver.
type Native struct {
	// Client returns the signed-in account's client, or nil signed out.
	Client func() PlayerAPI
	Solver Solver
	// Dir keeps the signature timestamp of each player across runs, so a
	// player parsed on an earlier run is not downloaded again for it.
	Dir  string
	HTTP *http.Client

	mu     sync.Mutex
	player playerInfo
	// premium is what the account has shown itself to be: 1 Premium, -1
	// not, 0 not yet known. It belongs to one client, and a sign-in makes
	// another.
	premium         int
	premiumOf       PlayerAPI
	lacking         int
	notPremiumUntil time.Time
	sts             map[string]int
}

type playerInfo struct {
	url string
	at  time.Time
}

// NewNative builds the adapter.
func NewNative(client func() PlayerAPI, solver Solver, dir string) *Native {
	return &Native{
		Client: client,
		Solver: solver,
		Dir:    dir,
		HTTP:   &http.Client{Timeout: 30 * time.Second},
	}
}

var _ Resolver = (*Native)(nil)

func (n *Native) Name() string { return "native" }

const (
	// playerFresh is how long the player's address is trusted. YouTube moves
	// to a new player every few days; a stale one still solves, until the
	// old player's functions stop matching the new addresses.
	playerFresh = 30 * time.Minute
	// notPremiumFor is how long an account that showed no subscriber
	// formats is left to the fallback before it is asked again.
	notPremiumFor = 30 * time.Minute
	// lackingBeforeVerdict is how many answers without subscriber formats
	// make an account of unknown standing "not Premium": a single track
	// without them (a podcast, an upload) proves nothing.
	lackingBeforeVerdict = 3
	userAgent            = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36"
)

var (
	rePlayerID = regexp.MustCompile(`player\\?/([0-9a-fA-F]{8})\\?/`)
	reSTS      = regexp.MustCompile(`(?:signatureTimestamp|sts)\s*:\s*([0-9]{5})`)
)

func (n *Native) Resolve(ctx context.Context, videoID string) (domain.Stream, Quality, error) {
	if !videoIDChars.MatchString(videoID) {
		return domain.Stream{}, Quality{}, ErrUnavailable
	}
	client := n.Client()
	if client == nil || !client.Authenticated() {
		return domain.Stream{}, Quality{}, ErrNotApplicable
	}
	if !n.mayServe(client) {
		return domain.Stream{}, Quality{}, ErrNotApplicable
	}

	playerURL, err := n.playerURL(ctx)
	if err != nil {
		return domain.Stream{}, Quality{}, classify(err)
	}
	sts, err := n.timestamp(ctx, playerURL)
	if err != nil {
		return domain.Stream{}, Quality{}, classify(err)
	}

	raw, err := client.Call(ctx, "player", map[string]any{
		"videoId": videoID,
		"playbackContext": map[string]any{
			"contentPlaybackContext": map[string]any{
				"html5Preference":    "HTML5_PREF_WANTS",
				"signatureTimestamp": sts,
			},
		},
		"contentCheckOk": true,
		"racyCheckOk":    true,
	})
	if err != nil {
		return domain.Stream{}, Quality{}, classify(err)
	}
	var resp playerResponse
	if err := json.Unmarshal(raw, &resp); err != nil {
		return domain.Stream{}, Quality{}, fmt.Errorf("native: read the player response: %w", err)
	}
	if err := resp.playable(); err != nil {
		return domain.Stream{}, Quality{}, err
	}

	premium := resp.offersPremium()
	if !n.judge(client, premium) {
		return domain.Stream{}, Quality{}, ErrNotApplicable
	}
	f := bestAdaptiveAudio(resp.StreamingData.AdaptiveFormats)
	if f == nil {
		return domain.Stream{}, Quality{}, ErrNoAudio
	}

	address, err := n.decipher(ctx, playerURL, f)
	if err != nil {
		return domain.Stream{}, Quality{}, err
	}

	bitrate := f.AverageBitrate
	if bitrate == 0 {
		bitrate = f.Bitrate
	}
	size, _ := strconv.ParseInt(f.ContentLength, 10, 64)
	durationMs, _ := strconv.ParseInt(f.ApproxDurationMs, 10, 64)
	q := describeQuality(f.Itag, bitrate, f.MimeType)
	return domain.Stream{
		Kind:       domain.StreamURL,
		VideoID:    videoID,
		URL:        address,
		MimeType:   f.MimeType,
		Bitrate:    bitrate,
		SizeBytes:  size,
		DurationMs: durationMs,
		ExpiresAt:  expiryOfURL(address, time.Now()),
	}, q, nil
}

// Close stops the solver's runtime, when it is one that runs.
func (n *Native) Close() {
	if c, ok := n.Solver.(interface{ Close() }); ok {
		c.Close()
	}
}

// Warm loads the current player into the solver ahead of the first song,
// which would otherwise wait for it: seconds under QuickJS for a player not
// parsed before, about one for one read back from disk. Signed out there is
// nothing to warm. Errors are left for the first resolve to meet.
func (n *Native) Warm(ctx context.Context) {
	client := n.Client()
	if client == nil || !client.Authenticated() || !n.mayServe(client) {
		return
	}
	playerURL, err := n.playerURL(ctx)
	if err != nil {
		return
	}
	if _, err := n.timestamp(ctx, playerURL); err != nil {
		return
	}
	source := func(ctx context.Context) ([]byte, error) { return n.fetch(ctx, playerURL) }
	_, _ = n.Solver.Solve(ctx, playerURL, source, nil, nil)
}

// mayServe is whether this account is worth a player request: not one
// that has shown it is not Premium, until that verdict has aged.
func (n *Native) mayServe(client PlayerAPI) bool {
	n.mu.Lock()
	defer n.mu.Unlock()
	if n.premiumOf != client {
		n.premiumOf, n.premium, n.lacking = client, 0, 0
	}
	if n.premium < 0 && time.Now().After(n.notPremiumUntil) {
		n.premium, n.lacking = 0, 0
	}
	return n.premium >= 0
}

// judge records what one answer showed of the account, and says whether
// to serve it. Subscriber formats prove Premium for good; an account of
// unknown standing whose answers keep lacking them is left to the
// fallback for a while.
func (n *Native) judge(client PlayerAPI, offersPremium bool) bool {
	n.mu.Lock()
	defer n.mu.Unlock()
	if n.premiumOf != client {
		return false
	}
	if offersPremium {
		n.premium, n.lacking = 1, 0
		return true
	}
	if n.premium > 0 {
		// A Premium account's standard formats need no token either.
		return true
	}
	n.lacking++
	if n.lacking >= lackingBeforeVerdict {
		n.premium = -1
		n.notPremiumUntil = time.Now().Add(notPremiumFor)
	}
	return false
}

// decipher puts the format's address through the player's functions.
func (n *Native) decipher(ctx context.Context, playerURL string, f *adaptiveFormat) (string, error) {
	raw, s, sp := f.URL, "", ""
	if raw == "" {
		cipher, err := url.ParseQuery(f.SignatureCipher)
		if err != nil {
			return "", fmt.Errorf("native: read the signature cipher: %w", err)
		}
		raw, s, sp = cipher.Get("url"), cipher.Get("s"), cipher.Get("sp")
	}
	u, err := url.Parse(raw)
	if err != nil || raw == "" {
		return "", fmt.Errorf("native: the format has no usable address")
	}
	q := u.Query()
	challenge := q.Get("n")

	var nList, sigList []string
	if challenge != "" {
		nList = []string{challenge}
	}
	if s != "" {
		sigList = []string{s}
	}
	if nList == nil && sigList == nil {
		return u.String(), nil
	}
	source := func(ctx context.Context) ([]byte, error) { return n.fetch(ctx, playerURL) }
	answers, err := n.Solver.Solve(ctx, playerURL, source, nList, sigList)
	if err != nil {
		return "", fmt.Errorf("native: %w", err)
	}
	if s != "" {
		if sp == "" {
			sp = "signature"
		}
		q.Set(sp, answers.Sig[s])
	}
	if challenge != "" {
		q.Set("n", answers.N[challenge])
	}
	u.RawQuery = q.Encode()
	return u.String(), nil
}

// playerURL is the current player's script, as YouTube's iframe API names
// it, in the "main" variant yt-dlp's solver is made for.
func (n *Native) playerURL(ctx context.Context) (string, error) {
	n.mu.Lock()
	cached := n.player
	n.mu.Unlock()
	if cached.url != "" && time.Since(cached.at) < playerFresh {
		return cached.url, nil
	}
	body, err := n.fetch(ctx, "https://www.youtube.com/iframe_api")
	if err != nil {
		if cached.url != "" {
			// An old player still solves; better than failing the song.
			return cached.url, nil
		}
		return "", err
	}
	m := rePlayerID.FindSubmatch(body)
	if m == nil {
		return "", errors.New("native: the iframe API named no player")
	}
	address := "https://www.youtube.com/s/player/" + string(m[1]) + "/player_ias.vflset/en_US/base.js"
	n.mu.Lock()
	n.player = playerInfo{url: address, at: time.Now()}
	n.mu.Unlock()
	return address, nil
}

// timestamp is the player's signature timestamp, which the player request
// has to carry. Kept on disk with the player, so a player parsed on an
// earlier run is not downloaded again for five digits.
func (n *Native) timestamp(ctx context.Context, playerURL string) (int, error) {
	n.mu.Lock()
	if n.sts == nil {
		n.sts = n.loadSTS()
	}
	sts, ok := n.sts[playerURL]
	n.mu.Unlock()
	if ok {
		return sts, nil
	}
	body, err := n.fetch(ctx, playerURL)
	if err != nil {
		return 0, err
	}
	m := reSTS.FindSubmatch(body)
	if m == nil {
		return 0, errors.New("native: the player has no signature timestamp")
	}
	sts, _ = strconv.Atoi(string(m[1]))
	n.mu.Lock()
	n.sts[playerURL] = sts
	n.saveSTS()
	n.mu.Unlock()
	return sts, nil
}

func (n *Native) stsFile() string { return filepath.Join(n.Dir, "signature-timestamps.json") }

func (n *Native) loadSTS() map[string]int {
	out := map[string]int{}
	if n.Dir == "" {
		return out
	}
	if b, err := os.ReadFile(n.stsFile()); err == nil {
		_ = json.Unmarshal(b, &out)
	}
	return out
}

// saveSTS writes the timestamps, the newest few: players come and go.
// Called with n.mu held.
func (n *Native) saveSTS() {
	if n.Dir == "" {
		return
	}
	if len(n.sts) > 8 {
		n.sts = map[string]int{n.player.url: n.sts[n.player.url]}
	}
	b, err := json.Marshal(n.sts)
	if err != nil {
		return
	}
	_ = os.MkdirAll(n.Dir, 0o700)
	tmp := n.stsFile() + ".part"
	if os.WriteFile(tmp, b, 0o600) == nil {
		_ = os.Rename(tmp, n.stsFile())
	}
}

func (n *Native) fetch(ctx context.Context, address string) ([]byte, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, address, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("User-Agent", userAgent)
	resp, err := n.HTTP.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("native: %s: HTTP %d", address, resp.StatusCode)
	}
	return io.ReadAll(io.LimitReader(resp.Body, 16<<20))
}

// playerResponse is the part of the player answer this reads.
type playerResponse struct {
	PlayabilityStatus struct {
		Status string `json:"status"`
		Reason string `json:"reason"`
	} `json:"playabilityStatus"`
	StreamingData struct {
		AdaptiveFormats []adaptiveFormat `json:"adaptiveFormats"`
	} `json:"streamingData"`
}

type adaptiveFormat struct {
	Itag             int    `json:"itag"`
	URL              string `json:"url"`
	SignatureCipher  string `json:"signatureCipher"`
	MimeType         string `json:"mimeType"`
	Bitrate          int    `json:"bitrate"`
	AverageBitrate   int    `json:"averageBitrate"`
	ContentLength    string `json:"contentLength"`
	ApproxDurationMs string `json:"approxDurationMs"`
	Type             string `json:"type"`
}

// playable reads the answer's verdict on the track. Only "gone for good" is
// ErrUnavailable, which ends the search; anything else (an age check, a
// region, a login wall) another client may still get past.
func (r *playerResponse) playable() error {
	switch r.PlayabilityStatus.Status {
	case "OK":
		return nil
	case "ERROR":
		return fmt.Errorf("%w: %s", ErrUnavailable, r.PlayabilityStatus.Reason)
	}
	return classify(fmt.Errorf("native: %s: %s", r.PlayabilityStatus.Status, r.PlayabilityStatus.Reason))
}

func (r *playerResponse) offersPremium() bool {
	for _, f := range r.StreamingData.AdaptiveFormats {
		if premiumItags[f.Itag] {
			return true
		}
	}
	return false
}

// bestAdaptiveAudio is the highest-bitrate audio-only format the relay can serve:
// progressive, not the segmented kind.
func bestAdaptiveAudio(formats []adaptiveFormat) *adaptiveFormat {
	var best *adaptiveFormat
	for i := range formats {
		f := &formats[i]
		if !strings.HasPrefix(f.MimeType, "audio/") || f.Type == "FORMAT_STREAM_TYPE_OTF" {
			continue
		}
		if f.URL == "" && f.SignatureCipher == "" {
			continue
		}
		if best == nil || f.Bitrate > best.Bitrate {
			best = f
		}
	}
	return best
}

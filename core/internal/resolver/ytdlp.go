package resolver

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"regexp"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"spotifier/internal/domain"
)

/*
The yt-dlp adapter.

ADR 0007 settled that decipher is delegated rather than implemented, and named
this the default for Premium tiers: the pure-Go library drives its own client
identity and cannot simply be handed account cookies — doing so turns a working
stream into a 403 — so it serves standard tiers only.

Two things this adapter must get right that the library one does not:

Cookies are passed as a file, not folded into a client context. yt-dlp applies
them to the player request the way a browser would, which is what makes the
subscriber-only tiers resolve at all.

The chosen format is audio-only and progressive. A DASH manifest would need a
segment fetcher the relay does not have, and the relay speaks plain ranges.
*/

// Ytdlp resolves by invoking the yt-dlp binary.
type Ytdlp struct {
	// Bin is the executable to run. Empty means "yt-dlp" on PATH.
	Bin string
	// CookiePath is a Netscape-format cookie file, or empty for anonymous
	// resolution. Without it only standard tiers are available.
	CookiePath string
	// Timeout bounds a single resolution. yt-dlp occasionally retries
	// internally, and an unbounded call would hang the player rather than
	// letting the failure ladder run.
	Timeout time.Duration
	// Deno is the JavaScript runtime yt-dlp solves YouTube's player
	// challenges with. Without one, signed-in requests fail outright ("The
	// page needs to be reloaded") and anonymous ones lose every audio format.
	// Empty leaves yt-dlp to find a runtime on PATH, which on most machines
	// does not exist.
	Deno string
	// Run, when set, runs yt-dlp in place of a child process. On Android
	// yt-dlp is Python inside the app, and there is no binary to start.
	Run RunFunc
	// JSRuntimes is given to yt-dlp as --js-runtimes when set, in place of
	// Deno, for a runtime other than deno (QuickJS on Android).
	JSRuntimes string

	once    sync.Once
	present bool

	// musicFailedAt is when the YouTube Music client was last found refusing
	// this listener: twice running it failed where the default way then
	// worked, as it does every track for a listener without Premium. Until
	// skipMusicClientFor has passed it is not asked again, so each track
	// costs one lookup rather than two. One refusal alone is a track it
	// cannot serve (a video, a podcast): the message is the same.
	musicFailedAt atomic.Int64
	musicRefusals atomic.Int32
	// now is the clock; tests replace it.
	now func() time.Time
}

// skipMusicClientFor is how long the YouTube Music client is left out after
// it failed for this listener.
const skipMusicClientFor = 10 * time.Minute

func (y *Ytdlp) clock() time.Time {
	if y.now != nil {
		return y.now()
	}
	return time.Now()
}

// tryMusicClient is whether asking as YouTube Music is worth it: it serves a
// signed-in listener's subscriber formats, and nothing at all without
// cookies (measured signed out: it failed after 4.9 s, and the default way
// then worked in 2.7 s), nor for a listener it has just refused.
func (y *Ytdlp) tryMusicClient() bool {
	if y.CookiePath == "" {
		return false
	}
	if _, err := os.Stat(y.CookiePath); err != nil {
		return false
	}
	at := y.musicFailedAt.Load()
	return at == 0 || y.clock().Sub(time.Unix(0, at)) >= skipMusicClientFor
}

// RunFunc runs yt-dlp with args and returns what it printed. A non-nil
// error means it failed, and stderr then says why.
type RunFunc func(ctx context.Context, args []string) (stdout, stderr []byte, err error)

// runProcess is RunFunc for the yt-dlp binary.
func runProcess(bin string) RunFunc {
	return func(ctx context.Context, args []string) ([]byte, []byte, error) {
		var stdout, stderr bytes.Buffer
		cmd := exec.CommandContext(ctx, bin, args...)
		cmd.Stdout = &stdout
		cmd.Stderr = &stderr
		err := cmd.Run()
		return stdout.Bytes(), stderr.Bytes(), err
	}
}

func (y *Ytdlp) runner() RunFunc {
	if y.Run != nil {
		return y.Run
	}
	return runProcess(y.binary())
}

// NewYtdlp builds the adapter. It does not probe for the binary; Available
// does that, once, on first use.
func NewYtdlp(bin, cookiePath string) *Ytdlp {
	return &Ytdlp{Bin: bin, CookiePath: cookiePath, Timeout: 45 * time.Second}
}

var _ Resolver = (*Ytdlp)(nil)

func (y *Ytdlp) Name() string { return "ytdlp" }

func (y *Ytdlp) binary() string {
	if y.Bin != "" {
		return y.Bin
	}
	return "yt-dlp"
}

// Available reports whether yt-dlp can be run at all.
//
// Probed once and cached: this is consulted when choosing a resolver at
// startup, and a missing binary must degrade to the pure-Go adapter rather
// than fail the process.
func (y *Ytdlp) Available() bool {
	// A runner the app supplies is there by construction; probing it would
	// start yt-dlp (Python, on Android) at launch for nothing.
	if y.Run != nil {
		return true
	}
	y.once.Do(func() {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_, _, err := y.runner()(ctx, []string{"--version"})
		y.present = err == nil
	})
	return y.present
}

// ytdlpFormat is the subset of yt-dlp's JSON this adapter reads.
type ytdlpFormat struct {
	Height    int     `json:"height"`
	URL       string  `json:"url"`
	Ext       string  `json:"ext"`
	ACodec    string  `json:"acodec"`
	VCodec    string  `json:"vcodec"`
	ABR       float64 `json:"abr"`
	TBR       float64 `json:"tbr"`
	FormatID  string  `json:"format_id"`
	Filesize  int64   `json:"filesize"`
	Protocol  string  `json:"protocol"`
	AudioOnly bool    `json:"-"`
}

type ytdlpInfo struct {
	Duration float64       `json:"duration"`
	Formats  []ytdlpFormat `json:"formats"`
}

/*
Asking as YouTube Music, then as anything.

By default yt-dlp tries several of YouTube's clients and starts by fetching
the watch page. Measured on one account: the default took 5.7 s and came
back with the standard format (251); restricted to the YouTube Music client
and skipping the page, it took 4.2-4.5 s and returned the subscriber format
(774). A track that client cannot serve — some videos, podcasts — is tried
again the default way before the resolver gives up.
*/
var musicClientArgs = []string{"--extractor-args", "youtube:player_client=web_music;player_skip=webpage,configs"}

// videoIDChars is YouTube's id alphabet. The id comes from a request's path
// and ends up in yt-dlp's arguments, so anything else is refused rather than
// passed on.
var videoIDChars = regexp.MustCompile(`^[A-Za-z0-9_-]+$`)

func (y *Ytdlp) Resolve(ctx context.Context, videoID string) (domain.Stream, Quality, error) {
	if !videoIDChars.MatchString(videoID) {
		return domain.Stream{}, Quality{}, ErrUnavailable
	}
	if y.Timeout > 0 {
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(ctx, y.Timeout)
		defer cancel()
	}
	if !y.tryMusicClient() {
		return y.resolveWith(ctx, videoID, nil, "https://www.youtube.com/watch?v=")
	}
	st, q, err := y.resolveWith(ctx, videoID, musicClientArgs, "https://music.youtube.com/watch?v=")
	if err == nil {
		y.musicRefusals.Store(0)
		return st, q, nil
	}
	if ctx.Err() != nil || errors.Is(err, ErrRateLimited) {
		return st, q, err
	}
	st, q, ferr := y.resolveWith(ctx, videoID, nil, "https://www.youtube.com/watch?v=")
	// The default way worked where YouTube Music's did not, a second time
	// running: this listener is not one it serves, so it is left out a while.
	if ferr == nil && y.musicRefusals.Add(1) >= 2 {
		y.musicRefusals.Store(0)
		y.musicFailedAt.Store(y.clock().UnixNano())
	}
	return st, q, ferr
}

func (y *Ytdlp) resolveWith(ctx context.Context, videoID string, extra []string, base string) (domain.Stream, Quality, error) {
	return y.resolveMedia(ctx, videoID, extra, base, nil)
}

// resolveMedia asks for the audio, or for the picture when a pick is given.
func (y *Ytdlp) resolveMedia(ctx context.Context, videoID string, extra []string, base string, pick *VideoPick) (domain.Stream, Quality, error) {
	video := pick != nil
	args := []string{
		"--dump-single-json",
		"--no-warnings",
		"--no-playlist",
		// Progressive audio only: the relay forwards byte ranges and has no
		// segment fetcher, so a DASH manifest would be unplayable here.
		"-f", "bestaudio[protocol^=http][acodec!=none][vcodec=none]/bestaudio",
	}
	if video {
		args[len(args)-1] = pick.selector()
	}
	if y.CookiePath != "" {
		if _, err := os.Stat(y.CookiePath); err == nil {
			args = append(args, "--cookies", y.CookiePath)
		}
	}
	switch {
	case y.JSRuntimes != "":
		args = append(args, "--js-runtimes", y.JSRuntimes)
	case y.Deno != "":
		args = append(args, "--js-runtimes", "deno:"+y.Deno)
	}
	args = append(args, extra...)
	args = append(args, base+videoID)

	stdout, stderr, err := y.runner()(ctx, args)
	if err != nil {
		return domain.Stream{}, Quality{}, classify(
			fmt.Errorf("yt-dlp: %v: %s", err, said(stderr, stdout)))
	}

	var info ytdlpInfo
	if err := json.Unmarshal(stdout, &info); err != nil {
		return domain.Stream{}, Quality{}, fmt.Errorf("yt-dlp: parse output: %w", err)
	}

	best := bestYtdlpAudio(info.Formats)
	mimeKind := "audio"
	if video {
		best = bestYtdlpVideo(info.Formats, *pick)
		mimeKind = "video"
	}
	if best == nil {
		return domain.Stream{}, Quality{}, ErrNoAudio
	}

	mimeType := fmt.Sprintf("audio/%s; codecs=%q", best.Ext, best.ACodec)
	if mimeKind == "video" {
		mimeType = "video/" + best.Ext
	}
	bitrate := int(best.ABR * 1000)
	if bitrate == 0 {
		bitrate = int(best.TBR * 1000)
	}
	codec := codecName(best.ACodec)

	return domain.Stream{
			Kind:       domain.StreamURL,
			VideoID:    videoID,
			URL:        best.URL,
			MimeType:   mimeType,
			Bitrate:    bitrate,
			SizeBytes:  best.Filesize,
			DurationMs: int64(info.Duration * 1000),
			// yt-dlp does not report the expiry; the URL carries it.
			ExpiresAt: expiryOfURL(best.URL, time.Now()),
		}, Quality{
			Label:   fmt.Sprintf("%s %d kbps", codec, bitrate/1000),
			Bitrate: bitrate,
			Codec:   codec,
			// The subscriber-only tiers. Reaching one is the whole reason this
			// adapter exists rather than the pure-Go one.
			Premium: best.FormatID == "774" || best.FormatID == "141",
		}, nil
}

// bestYtdlpAudio picks the highest-bitrate progressive audio-only format.
func bestYtdlpAudio(formats []ytdlpFormat) *ytdlpFormat {
	var best, lightest *ytdlpFormat
	limit := float64(ceiling()) / 1000
	for i := range formats {
		f := &formats[i]
		if f.URL == "" || f.ACodec == "" || f.ACodec == "none" {
			continue
		}
		if f.VCodec != "" && f.VCodec != "none" {
			continue // muxed: needless video bytes over the relay
		}
		if !strings.HasPrefix(f.Protocol, "http") {
			continue // a manifest the relay cannot serve
		}
		if lightest == nil || rate(f) < rate(lightest) {
			lightest = f
		}
		if limit > 0 && rate(f) > limit {
			continue
		}
		if best == nil || rate(f) > rate(best) {
			best = f
		}
	}
	// Nothing under the ceiling: the lightest there is.
	if best == nil {
		return lightest
	}
	return best
}

func rate(f *ytdlpFormat) float64 {
	if f.ABR > 0 {
		return f.ABR
	}
	return f.TBR
}

func codecName(acodec string) string {
	switch {
	case strings.HasPrefix(acodec, "opus"):
		return "Opus"
	case strings.HasPrefix(acodec, "mp4a"), strings.HasPrefix(acodec, "aac"):
		return "AAC"
	case acodec == "":
		return "Audio"
	default:
		return acodec
	}
}

// VideoPick narrows which picture stream is chosen. The zero value is what
// the Electron app's <video> element plays: the best picture up to 1080p in
// whatever codec YouTube offers, usually VP9 or AV1.
type VideoPick struct {
	// Codec is "" for any, or "h264": the one codec every Windows install
	// decodes without an extension from the Store, which is what a client
	// decoding through Media Foundation needs.
	Codec string
	// MaxHeight caps the picture; 0 means 1080.
	MaxHeight int
}

func (p VideoPick) height() int {
	if p.MaxHeight <= 0 || p.MaxHeight > 1080 {
		return 1080
	}
	return p.MaxHeight
}

func (p VideoPick) accepts(f *ytdlpFormat) bool {
	if f.Height <= 0 || f.Height > p.height() {
		return false
	}
	if p.Codec == "h264" {
		return f.Ext == "mp4" && strings.HasPrefix(f.VCodec, "avc1")
	}
	return f.Ext == "mp4" || f.Ext == "webm"
}

func (p VideoPick) selector() string {
	filter := fmt.Sprintf("[protocol^=http][height<=%d]", p.height())
	if p.Codec == "h264" {
		filter += "[vcodec^=avc1]"
	}
	return "bestvideo" + filter + "/best" + filter
}

// ResolveVideo supplies a progressive picture stream. The UI mutes it and
// retains the native audio engine as the single playback clock and sound source.
func (y *Ytdlp) ResolveVideo(ctx context.Context, id string) (domain.Stream, error) {
	return y.ResolveVideoAs(ctx, id, VideoPick{})
}

// ResolveVideoAs is ResolveVideo for a client that cannot play every codec.
func (y *Ytdlp) ResolveVideoAs(ctx context.Context, id string, pick VideoPick) (domain.Stream, error) {
	if !videoIDChars.MatchString(id) {
		return domain.Stream{}, ErrUnavailable
	}
	if y.Timeout > 0 {
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(ctx, y.Timeout)
		defer cancel()
	}
	type attempt struct {
		extra []string
		base  string
	}
	music := attempt{musicClientArgs, "https://music.youtube.com/watch?v="}
	anything := attempt{nil, "https://www.youtube.com/watch?v="}
	order := []attempt{music, anything}
	if pick.Codec != "" {
		// Measured signed out: the YouTube Music client offers one H.264
		// picture, 360p with the sound muxed in, and being a valid answer it
		// ended the search there. The default clients list H.264 up to 1080p.
		order = []attempt{anything, music}
	}
	st, _, err := y.resolveMedia(ctx, id, order[0].extra, order[0].base, &pick)
	if err != nil && ctx.Err() == nil && !errors.Is(err, ErrRateLimited) {
		st, _, err = y.resolveMedia(ctx, id, order[1].extra, order[1].base, &pick)
	}
	return st, err
}

func bestYtdlpVideo(formats []ytdlpFormat, pick VideoPick) *ytdlpFormat {
	var best *ytdlpFormat
	for i := range formats {
		f := &formats[i]
		if f.URL == "" || f.VCodec == "" || f.VCodec == "none" || !strings.HasPrefix(f.Protocol, "http") || !pick.accepts(f) {
			continue
		}
		if best == nil || f.Height > best.Height || (f.Height == best.Height && f.TBR > best.TBR) {
			best = f
		}
	}
	return best
}

// saidMost is how much of what yt-dlp printed an error carries.
const saidMost = 400

/*
said is what yt-dlp gave as the reason it failed: its error output. Where that
is empty, only how much it printed otherwise is said, never what: its output
is the track's JSON, whose stream addresses are signed for this machine, and
an error goes to the log and to the screen. A failure that printed nothing at
all says so, rather than ending in a colon that reads as a lost message.
*/
func said(stderr, stdout []byte) string {
	text := strings.TrimSpace(string(stderr))
	if len(text) > saidMost {
		// Cut by bytes, so a letter cut in two is dropped.
		text = "…" + strings.ToValidUTF8(text[len(text)-saidMost:], "")
	}
	switch {
	case text != "":
		return text
	case len(bytes.TrimSpace(stdout)) > 0:
		return fmt.Sprintf("nothing on stderr; %d bytes on stdout", len(stdout))
	default:
		return "yt-dlp printed nothing"
	}
}

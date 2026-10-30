// Package run is the core: every plane built and served on one address.
//
// The desktop's sidecar binary (cmd/spotifier) and the Android library
// (mobile) both start it, from flags and from a call respectively, so the
// two can never drift apart in what they build.
package run

import (
	"context"
	"crypto/subtle"
	"errors"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"path/filepath"
	"strings"
	"time"

	"spotifier/internal/account"
	"spotifier/internal/api"
	"spotifier/internal/audiocache"
	"spotifier/internal/catalog"
	"spotifier/internal/control"
	"spotifier/internal/domain"
	"spotifier/internal/innertube"
	"spotifier/internal/jsc"
	"spotifier/internal/loudness"
	"spotifier/internal/lyrics"
	"spotifier/internal/mixes"
	"spotifier/internal/obs"
	"spotifier/internal/ratelimit"
	"spotifier/internal/report"
	"spotifier/internal/resolver"
	"spotifier/internal/session"
)

// Config is what the core is started with. The desktop fills it from flags.
type Config struct {
	// Addr is the listen address; port 0 lets the system pick.
	Addr string
	// CatalogMode is auto, innertube or fixture.
	CatalogMode string
	FixtureDir  string
	// AccountScope is the desktop account scope for delayed play reports.
	AccountScope string
	CredPath     string
	DBPath       string
	// ResolverMode is auto, native, ytdlp or library.
	ResolverMode string
	// YtdlpBin and Deno are the programs the yt-dlp resolver runs. Ignored
	// when YtdlpRun is set.
	YtdlpBin string
	Deno     string
	// YtdlpRun runs yt-dlp somewhere other than a child process, as on
	// Android, where it is Python inside the app. JSRuntimes is then what
	// yt-dlp is given as --js-runtimes.
	YtdlpRun   resolver.RunFunc
	JSRuntimes string
	// CacheDir is the song cache; empty means beside the credentials.
	CacheDir string
	// ClientToken is the per-launch secret the app sends as
	// api.ClientTokenHeader. The desktop's core asks for it on video routes.
	ClientToken string
	// RequireToken refuses every request without ClientToken, where other
	// programs on the machine must not reach the core: on Android any app
	// can connect to the loopback address, and the core acts for the
	// signed-in account.
	RequireToken bool
}

// Run serves the core until ctx ends, then saves where the listener is and
// returns. listening is called once with the bound address, before the
// first request can arrive.
func Run(ctx context.Context, cfg Config, log *slog.Logger, listening func(addr string)) error {
	rec := obs.NewRecorder()
	deps := api.Deps{AccountScope: cfg.AccountScope, ClientToken: cfg.ClientToken, Recorder: rec, Log: log}

	/*
	 * One pace for everything this process asks of YouTube.
	 *
	 * Installed before any client is built: the catalog's, the account's and
	 * every one a sign-in builds later share one Governor and one scraped
	 * config, and every call they make is logged.
	 */
	innertube.SetDefaultGovernor(ratelimit.API)
	innertube.ShareConfig()
	upstream := newUpstreamLog(log)
	innertube.SetObserver(upstream.record)
	deps.APIGovernor, deps.StreamGovernor = ratelimit.API, ratelimit.Streams
	ratelimit.API.OnCooldown(func(d time.Duration) {
		log.Warn("YouTube asked us to slow down; pausing requests", "for", d.Round(time.Second))
	})
	ratelimit.Streams.OnCooldown(func(d time.Duration) {
		log.Warn("YouTube is rate-limiting stream lookups; pausing them", "for", d.Round(time.Second))
	})

	// The Control plane is ours and needs no credentials. If it cannot open,
	// browsing and playback still work; only the statistics surfaces and the
	// history-backed library sorts go missing.
	ctrl, err := control.Open(context.Background(), cfg.DBPath)
	if err != nil {
		log.Warn("control plane unavailable; stats and added/recents sorts disabled", "err", err)
	} else {
		defer ctrl.Close()
		deps.Control = ctrl
		// Resolved URLs outlive a restart, so replaying within their six
		// hours skips yt-dlp.
		deps.URLs = ctrl
	}
	// Answers read from YouTube are kept, in a file beside the account's
	// database, so repeat views and restarts do not ask for them again. The
	// desktop shell deletes that file when the account signs out or is
	// removed; it is separate so the listening history can stay.
	responsesPath := filepath.Join(filepath.Dir(cfg.DBPath), control.ResponsesFile)
	if kept, err := control.OpenResponses(context.Background(), responsesPath); err != nil {
		log.Warn("response cache file unavailable; keeping answers in memory only", "err", err)
		deps.Responses = api.NewResponseCache(nil, log)
	} else {
		defer kept.Close()
		deps.Responses = api.NewResponseCache(kept, log)
	}

	// Credentials are optional and may arrive later: signing in happens while
	// this process is running. The store re-reads the file on demand, which is
	// what lets a sign-in take effect without restarting playback.
	acct := account.New(cfg.CredPath, rec, ctrl)
	credErr := acct.Reload()
	// The session is checked once per launch rather than trusted from the
	// last one: a sign-in that expired while the app was closed must show.
	deps.Responses.Clear(context.Background(), "me|state")
	if !acct.SignedIn() {
		// No account now: nothing kept for one may be shown, even if the
		// shell could not delete the file.
		api.ClearSignedOut(context.Background(), deps.Responses)
	}
	deps.Account = acct

	// The catalog client is built from whatever credentials exist now. It is
	// separate from the Identity plane on purpose: public metadata does not
	// need an account, so browsing keeps working while signed out.
	creds, _ := innertube.LoadCredentials(cfg.CredPath)

	// Audio on disk beside the rest of the app's data, so a track played or
	// prefetched once starts without resolving. The cap is the client's to
	// set; this default holds a few hundred songs.
	cacheDir := cfg.CacheDir
	if cacheDir == "" {
		cacheDir = filepath.Join(filepath.Dir(cfg.CredPath), "audio-cache")
	}
	if cache, err := audiocache.New(cacheDir, 2<<30); err != nil {
		log.Warn("audio cache unavailable; every play streams from upstream", "err", err)
	} else {
		deps.Audio = cache
	}

	/*
	 * The catalog reads through the account's current client, so a session
	 * the shell refreshes (auth/reload) or ends (sign-out) applies to browsing
	 * at once instead of at the next restart. Signed out, a client without
	 * credentials serves public metadata.
	 */
	public := innertubeClient(nil)
	currentClient := func() *innertube.Client {
		if c := acct.Current().Client; c != nil {
			return c
		}
		return public
	}
	/*
	 * Choose a stream resolver.
	 *
	 * The native adapter first, for a Premium account: one player request
	 * and a solver that keeps the player parsed. yt-dlp next, for everyone
	 * else, because it applies account cookies the way a browser does and
	 * reaches other clients. The pure-Go library drives its own client
	 * identity and cannot be handed those cookies — doing so turns a working
	 * stream into a 403 — so it serves standard tiers and stands last.
	 */
	signedInClient := func() resolver.PlayerAPI {
		if c := acct.Current().Client; c != nil {
			return c
		}
		return nil
	}
	var native *resolver.Native
	deps.Resolver, native = buildResolver(cfg, creds, signedInClient, cacheDir, log)
	if native != nil {
		defer native.Close()
	}

	switch cfg.CatalogMode {
	case "fixture":
		f, err := fixture(cfg.FixtureDir, rec, log)
		if err != nil {
			return err
		}
		deps.Catalog = f
	default: // innertube, auto
		deps.Catalog = catalog.NewInnerTubeFrom(currentClient, rec)
	}

	/*
	 * Lyrics.
	 *
	 * The YouTube source needs the credentialed client, so it exists only
	 * while signed in. The timed source is a third party and is constructed
	 * unconditionally but contacted only when the client asks for timings,
	 * which the user controls in Settings.
	 */
	if client := acct.Current().Client; client != nil {
		deps.Lyrics = &lyrics.Service{
			Primary: lyrics.NewInnerTube(client, rec),
			Timed:   lyrics.NewLRCLib(),
		}
	} else {
		deps.Lyrics = &lyrics.Service{Timed: lyrics.NewLRCLib()}
	}

	/*
	 * Loudness.
	 *
	 * Signed in only, because the figure comes from a player response and an
	 * anonymous one carries no Premium formats worth normalising. Absent, the
	 * client measures as it plays, which is what it did before.
	 */
	if client := acct.Current().Client; client != nil {
		deps.Loudness = &loudness.Service{Client: client}
	}

	// Mixes need both halves: our history for the seeds, the catalog for the
	// radio that expands them.
	if deps.Control != nil && deps.Catalog != nil {
		deps.Mixes = mixes.New(deps.Control, deps.Catalog)
	}

	// Playback state is authoritative here rather than in the UI, so it
	// survives the window closing and can move between devices.
	var sink session.LogSink
	if ctrl != nil {
		sink = control.NewSink(ctrl, log)
	}
	deps.Session = session.NewHub(nil, session.DefaultSettings(), sink)

	/*
	 * Pick the listener up where they left off.
	 *
	 * Restored before the server starts listening, so the first client to
	 * connect receives the restored queue in its very first projection and
	 * never renders an empty player that then fills in.
	 *
	 * Paused, always — see internal/session/resume.go.
	 */
	var keeper *session.Keeper
	if ctrl != nil {
		keeper = &session.Keeper{Store: ctrl, UserID: control.DefaultUserID, Log: log}
		if snap := session.LoadSnapshot(context.Background(), ctrl, control.DefaultUserID); snap != nil {
			deps.Session.Restore(snap)
			log.Info("resumed where you left off",
				"track", snap.Tracks[snap.Index].Title,
				"at", time.Duration(snap.PositionMs)*time.Millisecond,
				"queued", len(snap.Tracks))
		}
	}

	deps.Resume = keeper

	/*
	 * Telling YouTube what was played.
	 *
	 * Off unless asked for, and the only thing in this program that writes to
	 * the account: it is what buys watch history, recommendations, and
	 * picking a track back up on another device — and it is also the one
	 * thing that stops Spotifier being invisible.
	 */
	if client := acct.Current().Client; client != nil {
		deps.Report = &report.Reporter{Client: client, Log: log}
	}

	apiServer := api.New(deps)
	// Any answer from YouTube ends an outage, not only the server's probe (#7).
	innertube.OnAnswered(apiServer.UpstreamAnswered)

	// A failed listener ends the run as a stop would, so the resume point is
	// still saved on the way out.
	ctx, stop := context.WithCancelCause(ctx)
	defer stop(nil)

	var handler http.Handler = apiServer
	if cfg.RequireToken {
		if cfg.ClientToken == "" {
			return errors.New("a client token is required but none was given")
		}
		handler = requireToken(cfg.ClientToken, apiServer)
	}

	srv := &http.Server{
		Addr:              cfg.Addr,
		Handler:           handler,
		ReadHeaderTimeout: 10 * time.Second,
		// Requests end when the run does. The session's event stream stays
		// open for as long as its client listens, and without this the
		// shutdown below waited out its whole timeout on it.
		BaseContext: func(net.Listener) context.Context { return ctx },
	}

	listener, err := net.Listen("tcp", cfg.Addr)
	if err != nil {
		return fmt.Errorf("listen: %w", err)
	}
	defer listener.Close()

	// The bound address, not the requested one: with port 0 the system picks,
	// and the app reads its choice from this line.
	log.Info("spotifier listening",
		"addr", listener.Addr().String(),
		"catalog", cfg.CatalogMode,
		"resolver", deps.Resolver.Name(),
		"signedIn", acct.SignedIn(),
	)
	if credErr != nil {
		log.Warn("no credentials; identity routes disabled", "err", credErr)
	}
	if listening != nil {
		listening(listener.Addr().String())
	}

	go upstream.run(ctx)

	go func() {
		if err := srv.Serve(listener); err != nil && !errors.Is(err, http.ErrServerClosed) {
			stop(fmt.Errorf("serve: %w", err))
		}
	}()

	// Reports playback upstream while it is switched on. Subscribed like any
	// other consumer, so neither the Hub nor the reporter knows about the
	// other — Run is the only place that knows both exist.
	if deps.Report != nil {
		go watchAndReport(ctx, deps.Session, deps.Report, log)
	}
	if deps.Audio != nil {
		go prefetchQueue(ctx, deps.Session, apiServer)
	}
	// Keeps the queue from running out, from YouTube's own radio.
	go apiServer.RunAutoplay(ctx)
	// The first song should not wait for the player to be parsed.
	if native != nil {
		go native.Warm(ctx)
	}

	// Records where the listener is as they go, and once more on the way out.
	// That last write is the one that matters: it captures the position at
	// the moment of closing, which is the one anybody would notice being
	// wrong.
	if keeper != nil {
		saved := make(chan struct{})
		go func() {
			keeper.Run(ctx, deps.Session)
			close(saved)
		}()
		defer func() { <-saved }()
	}

	<-ctx.Done()
	log.Info("shutting down")
	shutCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	_ = srv.Shutdown(shutCtx)
	if cause := context.Cause(ctx); !errors.Is(cause, context.Canceled) {
		return cause
	}
	return nil
}

// requireToken answers 403 to any request that does not carry token.
func requireToken(token string, next http.Handler) http.Handler {
	want := []byte(token)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if subtle.ConstantTimeCompare([]byte(r.Header.Get(api.ClientTokenHeader)), want) != 1 {
			http.Error(w, "forbidden", http.StatusForbidden)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func innertubeClient(creds *innertube.Credentials) *innertube.Client {
	opts := []innertube.Option{innertube.WithLocale("en", "US")}
	if creds != nil {
		opts = append(opts, innertube.WithCredentials(creds))
	}
	return innertube.New(opts...)
}

func fixture(dir string, rec *obs.Recorder, log *slog.Logger) (catalog.Catalog, error) {
	f := catalog.NewFixture(dir, rec)
	names, err := f.Available()
	if err != nil {
		return nil, fmt.Errorf("fixture mode needs recordings: %w (run: go run ./cmd/record)", err)
	}
	log.Info("fixture catalog", "recordings", len(names))
	return f, nil
}

/*
buildResolver picks the stream resolver and prepares what it needs.

"auto" prefers yt-dlp when it can be run and falls back to the pure-Go
library otherwise, so a machine without yt-dlp still plays standard tiers
rather than failing to start.
*/
func buildResolver(cfg Config, creds *innertube.Credentials, client func() resolver.PlayerAPI, cacheDir string, log *slog.Logger) (resolver.Resolver, *resolver.Native) {
	library := resolver.NewLibrary()
	if cfg.ResolverMode == "library" {
		return library, nil
	}

	// The native adapter needs a JavaScript runtime for the challenges: the
	// one yt-dlp is given.
	var native *resolver.Native
	spec := cfg.JSRuntimes
	if spec == "" && cfg.Deno != "" {
		spec = "deno:" + cfg.Deno
	}
	if rt, err := jsc.ParseRuntime(spec); err == nil {
		// Beside the song cache, as disposable as it is.
		dir := filepath.Join(filepath.Dir(cacheDir), "challenge-solver")
		native = resolver.NewNative(client, jsc.New(rt, dir, log), dir)
		log.Info("native resolver ready", "runtime", rt.Kind)
	} else if spec != "" {
		log.Warn("no native resolver: the JavaScript runtime is not one it knows", "err", err)
	}
	if cfg.ResolverMode == "native" {
		if native == nil {
			return resolver.NewChain(nil, nil, log), nil
		}
		return resolver.NewChain(native, nil, log), native
	}

	/*
	 * yt-dlp reads cookies from a file, so the session is exported to one.
	 *
	 * It lives beside credentials.json under one fixed name, overwritten on
	 * every start. It used to go in a fresh folder in the system temp
	 * directory, removed on a clean exit — but a process that is killed never
	 * exits cleanly, and every such run left another plain-text copy of a live
	 * session behind: over a hundred had accumulated. Next to the credentials
	 * it is the same secret in the same protected place, and there is only
	 * ever one.
	 */
	cookiePath := ""
	if creds != nil && cfg.CredPath != "" {
		path := filepath.Join(filepath.Dir(cfg.CredPath), "yt-dlp-cookies.txt")
		if n, err := innertube.WriteCookieFile(creds, path); err != nil {
			log.Warn("could not export cookies for yt-dlp; it will resolve anonymously", "err", err)
		} else {
			cookiePath = path
			log.Debug("exported cookies for yt-dlp", "count", n)
		}
	}

	yt := resolver.NewYtdlp(cfg.YtdlpBin, cookiePath)
	switch {
	case cfg.YtdlpRun != nil:
		yt.Run = cfg.YtdlpRun
		yt.JSRuntimes = cfg.JSRuntimes
		log.Info("yt-dlp runs in-process", "js-runtimes", cfg.JSRuntimes)
	case cfg.Deno == "":
		log.Warn("no JavaScript runtime given to yt-dlp; playback works only if deno is on PATH")
	default:
		yt.Deno = cfg.Deno
		log.Info("yt-dlp will solve player challenges with deno", "path", cfg.Deno)
	}
	if cfg.ResolverMode == "ytdlp" {
		return resolver.NewChain(yt, nil, log), nil
	}
	rest := resolver.Resolver(library)
	if yt.Available() {
		rest = resolver.NewChain(yt, library, log)
	} else {
		log.Info("yt-dlp not found; the pure-Go resolver serves whom the native one cannot (standard tiers only)")
	}
	if native == nil {
		return rest, nil
	}
	return resolver.NewChain(native, rest, log), native
}

/*
watchAndReport pings YouTube as the listener plays.

Projections say what is playing and where; this turns that into the cadence
YouTube's own players use. The position is read from the projection rather
than interpolated, so a paused player reports the same figure and the reporter
drops it — pausing stops the reporting without needing to be told.

A pause reports once at that moment, or everything since the last tick would
go unreported for as long as the pause lasts, and for good if it is the last.
*/
func watchAndReport(ctx context.Context, hub *session.Hub, rep *report.Reporter, log *slog.Logger) {
	updates, cancel := hub.Subscribe()
	defer cancel()

	tick := time.NewTicker(report.Interval)
	defer tick.Stop()

	var latest domain.Session
	for {
		select {
		case <-ctx.Done():
			return

		case p, ok := <-updates:
			if !ok {
				return
			}
			paused := latest.State == domain.StatePlaying && p.State.State == domain.StatePaused
			latest = p.State
			// A track change needs its tracking URLs before anything can be
			// reported against it, so fetch them as soon as it changes.
			if !rep.Enabled() {
				continue
			}
			if t := current(latest); t != "" {
				if err := rep.Track(ctx, t); err != nil {
					log.Debug("report: could not prepare", "err", err)
				}
				if paused {
					rep.Progress(ctx, t, latest.PositionMs)
				}
			}

		case <-tick.C:
			if latest.State != domain.StatePlaying {
				continue
			}
			if t := current(latest); t != "" {
				rep.Progress(ctx, t, latest.PositionMs)
			}
		}
	}
}

/*
prefetchQueue keeps what is about to play on disk.

Spotify starts downloading the next track while the current one is still
playing, and YouTube Music preloads the next track's player. This does both
for the playing track and the three after it, so a skip, a crossfade or the
queue simply moving on never waits for a resolution. The session restored at
launch comes through here too, so pressing play after opening the app is
immediate.
*/
func prefetchQueue(ctx context.Context, hub *session.Hub, srv *api.Server) {
	updates, cancel := hub.Subscribe()
	defer cancel()

	// Every projection would otherwise re-ask; only a change in what is
	// coming up is worth acting on.
	last := ""
	handle := func(s domain.Session, offline bool) {
		if s.Queue.Index < 0 || s.Queue.Index >= len(s.Queue.Items) {
			return
		}
		ids := srv.QueueToFetch(s)
		// Coming back online asks again: nothing could be fetched meanwhile.
		key := strings.Join(ids, ",") + fmt.Sprint(offline)
		if key == last {
			return
		}
		last = key
		srv.PrefetchQueue(ids)
	}
	first := hub.Projection()
	handle(first.State, first.Offline)
	for {
		select {
		case <-ctx.Done():
			return
		case p, ok := <-updates:
			if !ok {
				return
			}
			handle(p.State, p.Offline)
		}
	}
}

// current is the Track the session is on, or "" when the queue is empty.
func current(s domain.Session) string {
	if s.Queue.Index < 0 || s.Queue.Index >= len(s.Queue.Items) {
		return ""
	}
	return s.Queue.Items[s.Queue.Index].ID
}

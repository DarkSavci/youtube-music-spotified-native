// Package mobile starts the core inside an Android app.
//
// gomobile binds it into an AAR (scripts/build-core-android.ps1). What the
// desktop passes as flags is a Config here, and yt-dlp, which on Android is
// Python inside the app rather than a program to start, comes in as a
// YtdlpRunner the app implements. Only types gomobile can carry cross this
// boundary: strings, numbers, booleans, errors and interfaces of them.
package mobile

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sync"

	"spotifier/internal/api"
	"spotifier/internal/run"
)

// Config is what the app starts the core with.
type Config struct {
	// DataDir holds the credentials and the databases: the app's filesDir.
	DataDir string
	// CacheDir holds the song cache: the app's cacheDir, which Android may
	// clear when space runs short.
	CacheDir string
	// Resolver is auto, native, ytdlp or library.
	Resolver string
	// JSRuntimes is what yt-dlp is given as --js-runtimes, such as
	// "quickjs:/data/app/.../libqjs.so".
	JSRuntimes string
	Verbose    bool
}

// NewConfig is a Config with the defaults, for the app to fill in.
func NewConfig() *Config { return &Config{Resolver: "auto"} }

// YtdlpRunner runs yt-dlp. Args are a JSON array of strings, so no
// argument can be split into two whatever it holds. It returns what yt-dlp
// printed on stdout; a failure is an error whose message is what it printed
// on stderr.
type YtdlpRunner interface {
	Run(args string) (string, error)
}

// TokenHeader is the header every request to the core must carry, with
// Token's value.
const TokenHeader = api.ClientTokenHeader

var (
	mu      sync.Mutex
	runner  YtdlpRunner
	sink    LogSink
	stop    context.CancelFunc
	stopped chan struct{}
	token   string
)

// Token is the running core's secret, new at every Start: any app on the
// phone can connect to the loopback address, and only this one knows it.
// Empty while the core is stopped.
func Token() string {
	mu.Lock()
	defer mu.Unlock()
	return token
}

// SetYtdlp gives the core the app's yt-dlp. Called before Start; nil leaves
// only the pure-Go resolver.
func SetYtdlp(r YtdlpRunner) {
	mu.Lock()
	defer mu.Unlock()
	runner = r
}

// Start runs the core and returns the address it bound, once it is ready to
// answer. A second Start while one runs returns an error.
func Start(cfg *Config) (string, error) {
	mu.Lock()
	defer mu.Unlock()
	if stop != nil {
		return "", errors.New("the core is already running")
	}
	if cfg == nil || cfg.DataDir == "" {
		return "", errors.New("a data directory is needed")
	}

	level := slog.LevelInfo
	if cfg.Verbose {
		level = slog.LevelDebug
	}
	// Without the app's log, standard error: gomobile sends it to logcat.
	var handler slog.Handler = slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: level})
	if sink != nil {
		handler = newSinkHandler(sink, level)
	}
	log := slog.New(handler)
	slog.SetDefault(log)

	secret := make([]byte, 32)
	if _, err := rand.Read(secret); err != nil {
		return "", fmt.Errorf("make the client token: %w", err)
	}

	rc := run.Config{
		Addr:         "127.0.0.1:0",
		ClientToken:  hex.EncodeToString(secret),
		RequireToken: true,
		CatalogMode:  "auto",
		CredPath:     filepath.Join(cfg.DataDir, "credentials.json"),
		DBPath:       filepath.Join(cfg.DataDir, "spotifier.db"),
		ResolverMode: cfg.Resolver,
		JSRuntimes:   cfg.JSRuntimes,
	}
	if cfg.CacheDir != "" {
		rc.CacheDir = filepath.Join(cfg.CacheDir, "audio-cache")
	}
	if runner != nil {
		rc.YtdlpRun = bridge(runner)
	} else if rc.ResolverMode == "auto" {
		// Nothing to run yt-dlp with; without this the core would look for
		// a yt-dlp program, which an Android app does not have.
		rc.ResolverMode = "library"
	}

	ctx, cancel := context.WithCancel(context.Background())
	addr := make(chan string, 1)
	failed := make(chan error, 1)
	done := make(chan struct{})
	go func() {
		defer close(done)
		// A panic here would take the whole app down with it.
		defer func() {
			if p := recover(); p != nil {
				log.Error("core panicked", "panic", p)
				failed <- fmt.Errorf("the core stopped: %v", p)
			}
		}()
		if err := run.Run(ctx, rc, log, func(a string) { addr <- a }); err != nil {
			failed <- err
		}
	}()

	select {
	case a := <-addr:
		stop, stopped, token = cancel, done, rc.ClientToken
		return a, nil
	case err := <-failed:
		cancel()
		return "", err
	case <-done:
		cancel()
		return "", errors.New("the core stopped before it was ready")
	}
}

// Stop ends the core and returns once it has saved where the listener is.
func Stop() {
	mu.Lock()
	cancel, done := stop, stopped
	stop, stopped, token = nil, nil, ""
	mu.Unlock()
	if cancel == nil {
		return
	}
	cancel()
	<-done
}

// bridge turns the app's runner into the resolver's. The app's call cannot
// be cancelled, so a resolve that times out stops waiting for it and leaves
// it to finish on its own.
func bridge(r YtdlpRunner) func(context.Context, []string) ([]byte, []byte, error) {
	return func(ctx context.Context, args []string) ([]byte, []byte, error) {
		type result struct {
			out string
			err error
		}
		encoded, err := json.Marshal(args)
		if err != nil {
			return nil, nil, err
		}
		got := make(chan result, 1)
		go func() {
			out, err := r.Run(string(encoded))
			got <- result{out, err}
		}()
		select {
		case <-ctx.Done():
			return nil, nil, ctx.Err()
		case res := <-got:
			if res.err != nil {
				return []byte(res.out), []byte(res.err.Error()), res.err
			}
			return []byte(res.out), nil, nil
		}
	}
}

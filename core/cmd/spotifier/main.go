// Command spotifier runs the Go core.
//
// One binary, two modes:
//
//	spotifier                    # local: all planes in-process, the desktop sidecar
//	spotifier -catalog fixture   # local, offline: serves recorded responses
//
// Server mode, which lifts the Catalog and Control planes out while leaving
// Identity on the Device, is not implemented yet and is deliberately a later
// milestone rather than a different program.
package main

import (
	"context"
	"flag"
	"io"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	"spotifier/internal/run"
)

func main() {
	var (
		addr          = flag.String("addr", "127.0.0.1:8674", "listen address")
		catalogMode   = flag.String("catalog", "auto", "catalog source: auto | innertube | fixture")
		fixtureDir    = flag.String("fixtures", "testdata/fixtures", "fixture directory")
		accountScope  = flag.String("account-scope", "", "desktop account scope for delayed play reports")
		credPath      = flag.String("credentials", "credentials.json", "path to credentials")
		dbPath        = flag.String("db", "spotifier.db", "control-plane database")
		verbose       = flag.Bool("v", false, "debug logging")
		resolverMode  = flag.String("resolver", "auto", "stream resolver: auto | native | ytdlp | library")
		ytdlpBin      = flag.String("ytdlp", "", "path to yt-dlp (default: found on PATH)")
		denoBin       = flag.String("deno", "", "path to deno, the JavaScript runtime yt-dlp needs (default: found on PATH)")
		cacheDir      = flag.String("cache", "", "song cache directory (default: beside the credentials)")
		exitWithStdin = flag.Bool("exit-with-stdin", false, "shut down when stdin closes (the native app holds the pipe)")
		inspectPath   = flag.String("migrate-inspect", "", "print what another profile's database holds, as JSON, and exit")
		mergePath     = flag.String("migrate-from", "", "merge another profile's database into -db, print what was added as JSON, and exit")
	)
	flag.Parse()
	if *inspectPath != "" || *mergePath != "" {
		os.Exit(migrate(*inspectPath, *mergePath, *dbPath))
	}

	level := slog.LevelInfo
	if *verbose {
		level = slog.LevelDebug
	}
	log := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: level}))
	// Packages without a logger of their own (library, identity) log through
	// the default, which should read like the rest of the core's output.
	slog.SetDefault(log)

	cfg := run.Config{
		Addr:         *addr,
		CatalogMode:  *catalogMode,
		FixtureDir:   *fixtureDir,
		AccountScope: *accountScope,
		CredPath:     *credPath,
		DBPath:       *dbPath,
		ResolverMode: *resolverMode,
		YtdlpBin:     *ytdlpBin,
		Deno:         *denoBin,
		CacheDir:     *cacheDir,
		// The token comes through the environment rather than a flag, so it
		// does not show up in process listings.
		ClientToken: os.Getenv("SPOTIFIER_CLIENT_TOKEN"),
	}
	// Keep it out of the environment yt-dlp and deno inherit.
	_ = os.Unsetenv("SPOTIFIER_CLIENT_TOKEN")

	// Shut down cleanly so the desktop shell never leaves an orphaned sidecar.
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	if *exitWithStdin {
		go stopWhenStdinCloses(stop)
	}

	if err := run.Run(ctx, cfg, log, nil); err != nil {
		log.Error("core stopped", "err", err)
		stop()
		os.Exit(1)
	}
}

// stopWhenStdinCloses ends the run when the parent closes our stdin, which
// is how the native app asks for a clean stop: on Windows a killed process
// never runs its shutdown, and the final resume save is in that shutdown. A
// parent that crashes closes the pipe too, so no orphan is left behind.
func stopWhenStdinCloses(stop context.CancelFunc) {
	_, _ = io.Copy(io.Discard, os.Stdin)
	stop()
}

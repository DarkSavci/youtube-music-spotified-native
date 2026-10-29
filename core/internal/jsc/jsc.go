// Package jsc solves YouTube's player challenges: the sig and n functions a
// stream address has to be put through before it plays.
//
// The solving is yt-dlp-ejs's, the challenge solver yt-dlp itself uses
// (ejs/, Unlicense, bundling meriyah under ISC and astring under MIT), run by
// a JavaScript runtime the app ships: QuickJS-NG on Android, deno on the
// desktop. yt-dlp starts the runtime and parses the whole player, about
// 2.6 MB, for every song; that was six seconds a resolve under QuickJS.
// Here one process stays up, parses a player once, keeps its two functions,
// and a player parsed on an earlier run is read back from disk, so a solve
// costs a few milliseconds.
package jsc

import (
	"bufio"
	"context"
	"crypto/sha256"
	"embed"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"
)

//go:embed ejs/lib.min.js ejs/core.min.js worker.js
var files embed.FS

// keepPlayers is how many parsed players are kept on disk. YouTube moves to
// a new player every few days; each is a few megabytes.
const keepPlayers = 3

// idleStop is how long the runtime stays up with nothing to solve. QuickJS
// holds about 60 MB with a player loaded; reading the player back from disk
// takes about a second, and while music plays the next songs are resolved
// often enough to keep it up.
const idleStop = 10 * time.Minute

// Runtime is the JavaScript engine the solver runs in.
type Runtime struct {
	// Kind is "quickjs" (QuickJS-NG) or "deno".
	Kind string
	Path string
}

// ParseRuntime reads a runtime as yt-dlp's --js-runtimes names one:
// "quickjs:/path/to/qjs" or "deno:/path/to/deno".
func ParseRuntime(spec string) (Runtime, error) {
	kind, path, ok := strings.Cut(spec, ":")
	if !ok || path == "" {
		return Runtime{}, fmt.Errorf("jsc: %q is not kind:path", spec)
	}
	switch kind {
	case "quickjs", "deno":
		return Runtime{Kind: kind, Path: path}, nil
	}
	return Runtime{}, fmt.Errorf("jsc: unknown runtime %q", kind)
}

func (r Runtime) command(script string) *exec.Cmd {
	if r.Kind == "deno" {
		// No permissions are asked for: the worker reads stdin and writes
		// stdout, which deno allows without any.
		return exec.Command(r.Path, "run", "--quiet", "--no-prompt", "--no-config", script)
	}
	return exec.Command(r.Path, "--std", "--script", script)
}

// Answers maps each challenge to its solution.
type Answers struct {
	N   map[string]string `json:"n"`
	Sig map[string]string `json:"sig"`
}

// Solver answers challenges. It is safe for concurrent use; solves run one
// at a time, which costs nothing once a player is loaded.
type Solver struct {
	rt  Runtime
	dir string
	log *slog.Logger

	mu     sync.Mutex
	w      *worker
	loaded map[string]bool
	idle   *time.Timer
}

// New makes a solver that keeps its script and parsed players in dir.
func New(rt Runtime, dir string, log *slog.Logger) *Solver {
	if log == nil {
		log = slog.Default()
	}
	return &Solver{rt: rt, dir: dir, log: log}
}

// Solve answers n and sig challenges against the player at key, its URL.
// source fetches the player's script, and is called only when the player
// has not been parsed before.
func (s *Solver) Solve(ctx context.Context, key string, source func(context.Context) ([]byte, error), n, sig []string) (Answers, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	defer s.restartIdle()

	if err := s.start(); err != nil {
		return Answers{}, err
	}
	if !s.loaded[key] {
		if err := s.load(ctx, key, source); err != nil {
			return Answers{}, err
		}
		s.loaded[key] = true
	}

	var out struct {
		Answers
		Error string `json:"error"`
	}
	if err := s.call(ctx, map[string]any{"op": "solve", "key": key, "n": n, "sig": sig}, &out); err != nil {
		return Answers{}, err
	}
	if out.Error != "" {
		return Answers{}, fmt.Errorf("jsc: solve: %s", out.Error)
	}
	for _, c := range n {
		if out.N[c] == "" {
			return Answers{}, fmt.Errorf("jsc: no answer to the n challenge %q", c)
		}
	}
	for _, c := range sig {
		if out.Sig[c] == "" {
			return Answers{}, errors.New("jsc: no answer to a sig challenge")
		}
	}
	return out.Answers, nil
}

// Close stops the runtime.
func (s *Solver) Close() {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.idle != nil {
		s.idle.Stop()
	}
	s.stop()
}

// restartIdle stops the runtime idleStop after the last solve. Called with
// s.mu held.
func (s *Solver) restartIdle() {
	if s.idle != nil {
		s.idle.Stop()
	}
	s.idle = time.AfterFunc(idleStop, func() {
		s.mu.Lock()
		defer s.mu.Unlock()
		s.stop()
	})
}

// load parses the player into the worker, from disk when it was parsed on
// an earlier run.
func (s *Solver) load(ctx context.Context, key string, source func(context.Context) ([]byte, error)) error {
	saved := filepath.Join(s.dir, "players", playerFile(key))
	if pre, err := os.ReadFile(saved); err == nil {
		var out struct{ Error string }
		err := s.call(ctx, map[string]any{"op": "load", "key": key, "preprocessed": string(pre)}, &out)
		if err == nil && out.Error == "" {
			return nil
		}
		// A file a different solver wrote, or a torn one: parse afresh.
		s.log.Warn("jsc: a saved player would not load; parsing it again", "err", err, "said", out.Error)
		_ = os.Remove(saved)
		if err != nil {
			return err
		}
	}

	player, err := source(ctx)
	if err != nil {
		return fmt.Errorf("jsc: fetch the player: %w", err)
	}
	var out struct {
		Preprocessed string `json:"preprocessed"`
		Error        string `json:"error"`
	}
	if err := s.call(ctx, map[string]any{"op": "load", "key": key, "player": string(player)}, &out); err != nil {
		return err
	}
	if out.Error != "" {
		return fmt.Errorf("jsc: parse the player: %s", out.Error)
	}
	if err := writeFile(saved, []byte(out.Preprocessed)); err != nil {
		s.log.Warn("jsc: could not keep the parsed player", "err", err)
	}
	prune(filepath.Dir(saved), keepPlayers)
	return nil
}

// playerFile names a player's file by its URL, which has slashes in it.
func playerFile(key string) string {
	sum := sha256.Sum256([]byte(key))
	return hex.EncodeToString(sum[:8]) + ".js"
}

// call sends one request and reads its answer. A call cut short leaves the
// worker mid-answer, so it is stopped, and the next call starts another.
func (s *Solver) call(ctx context.Context, req any, out any) error {
	line, err := json.Marshal(req)
	if err != nil {
		return err
	}
	type reply struct {
		line []byte
		err  error
	}
	got := make(chan reply, 1)
	w := s.w
	go func() {
		if _, err := w.in.Write(append(line, '\n')); err != nil {
			got <- reply{err: err}
			return
		}
		b, err := w.out.ReadBytes('\n')
		got <- reply{b, err}
	}()
	select {
	case <-ctx.Done():
		s.stop()
		return ctx.Err()
	case r := <-got:
		if r.err != nil {
			s.stop()
			return fmt.Errorf("jsc: the runtime stopped: %w%s", r.err, w.said())
		}
		return json.Unmarshal(r.line, out)
	}
}

type worker struct {
	cmd    *exec.Cmd
	in     io.WriteCloser
	out    *bufio.Reader
	stderr *tail
}

// said is the end of what the runtime printed on stderr, for an error.
func (w *worker) said() string {
	if text := strings.TrimSpace(w.stderr.String()); text != "" {
		return ": " + text
	}
	return ""
}

func (s *Solver) start() error {
	if s.w != nil {
		return nil
	}
	script, err := s.script()
	if err != nil {
		return err
	}
	cmd := s.rt.command(script)
	in, err := cmd.StdinPipe()
	if err != nil {
		return err
	}
	out, err := cmd.StdoutPipe()
	if err != nil {
		return err
	}
	stderr := &tail{}
	cmd.Stderr = stderr
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("jsc: start %s: %w", s.rt.Kind, err)
	}
	s.w = &worker{cmd: cmd, in: in, out: bufio.NewReaderSize(out, 1<<20), stderr: stderr}
	s.loaded = map[string]bool{}
	return nil
}

func (s *Solver) stop() {
	if s.w == nil {
		return
	}
	_ = s.w.in.Close()
	_ = s.w.cmd.Process.Kill()
	_ = s.w.cmd.Wait()
	s.w = nil
	s.loaded = nil
}

// script writes the solver and the worker to one file, the form both
// runtimes run, named by its contents so a new version never runs an old
// file.
func (s *Solver) script() (string, error) {
	var b strings.Builder
	for _, name := range []string{"ejs/lib.min.js", "ejs/core.min.js", "worker.js"} {
		part, err := files.ReadFile(name)
		if err != nil {
			return "", err
		}
		b.Write(part)
		b.WriteString("\n")
		if name == "ejs/lib.min.js" {
			// The library's exports, meriyah and astring, are what the core
			// expects as globals; yt-dlp joins the two the same way.
			b.WriteString("Object.assign(globalThis, lib);\n")
		}
	}
	sum := sha256.Sum256([]byte(b.String()))
	path := filepath.Join(s.dir, "solver-"+hex.EncodeToString(sum[:6])+".js")
	if _, err := os.Stat(path); err == nil {
		return path, nil
	}
	if err := writeFile(path, []byte(b.String())); err != nil {
		return "", fmt.Errorf("jsc: write the solver: %w", err)
	}
	return path, nil
}

// writeFile writes by a temporary file and a rename, so a reader never
// sees half a file.
func writeFile(path string, data []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return err
	}
	tmp := path + ".part"
	if err := os.WriteFile(tmp, data, 0o600); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}

// prune keeps the newest keep files in dir.
func prune(dir string, keep int) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}
	type file struct {
		path string
		at   int64
	}
	var all []file
	for _, e := range entries {
		if info, err := e.Info(); err == nil && !e.IsDir() && strings.HasSuffix(e.Name(), ".js") {
			all = append(all, file{filepath.Join(dir, e.Name()), info.ModTime().UnixNano()})
		}
	}
	sort.Slice(all, func(i, j int) bool { return all[i].at > all[j].at })
	for _, f := range all[min(keep, len(all)):] {
		_ = os.Remove(f.path)
	}
}

// tail keeps the last few kilobytes written to it.
type tail struct {
	mu  sync.Mutex
	buf []byte
}

func (t *tail) Write(p []byte) (int, error) {
	t.mu.Lock()
	defer t.mu.Unlock()
	t.buf = append(t.buf, p...)
	if over := len(t.buf) - 4096; over > 0 {
		t.buf = t.buf[over:]
	}
	return len(p), nil
}

func (t *tail) String() string {
	t.mu.Lock()
	defer t.mu.Unlock()
	return string(t.buf)
}

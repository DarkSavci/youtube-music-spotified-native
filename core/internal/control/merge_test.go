package control_test

import (
	"context"
	"database/sql"
	"os"
	"path/filepath"
	"testing"
	"time"

	"spotifier/internal/control"
	"spotifier/internal/domain"
)

// openAt is a store at a path the test chooses, so another store can merge it.
func openAt(t *testing.T, path string) *control.Store {
	t.Helper()
	s, err := control.Open(context.Background(), path)
	if err != nil {
		t.Fatalf("open: %v", err)
	}
	t.Cleanup(func() { s.Close() })
	return s
}

func record(t *testing.T, s *control.Store, plays ...control.Play) {
	t.Helper()
	if err := s.RecordPlays(context.Background(), control.DefaultUserID, plays); err != nil {
		t.Fatalf("record: %v", err)
	}
}

func playCount(t *testing.T, s *control.Store) int {
	t.Helper()
	sum, err := s.Summary(context.Background(), control.DefaultUserID, control.Last(24*time.Hour))
	if err != nil {
		t.Fatal(err)
	}
	return sum.Plays
}

func merge(t *testing.T, into *control.Store, from string, opts control.MergeOptions) control.Merged {
	t.Helper()
	got, err := into.MergeFrom(context.Background(), from, opts)
	if err != nil {
		t.Fatalf("merge: %v", err)
	}
	return got
}

// Both profiles were listened in. Neither history replaces the other, and
// bringing the same one across again adds nothing.
func TestMergeAddsTheOtherHistoryOnce(t *testing.T) {
	dir := t.TempDir()
	oldPath := filepath.Join(dir, "old", "spotifier.db")
	if err := os.MkdirAll(filepath.Dir(oldPath), 0o700); err != nil {
		t.Fatal(err)
	}
	old := openAt(t, oldPath)
	record(t, old,
		play("old-1", "t1", "Duman", "UC1", 3*time.Hour, 200_000),
		play("old-2", "t2", "Duman", "UC1", 2*time.Hour, 180_000),
		play("old-3", "t1", "Duman", "UC1", time.Hour, 200_000))
	mine := openAt(t, filepath.Join(dir, "mine.db"))
	record(t, mine,
		play("mine-1", "t1", "Duman", "UC1", 30*time.Minute, 200_000),
		play("mine-2", "t9", "Adamlar", "UC2", 20*time.Minute, 150_000))

	got := merge(t, mine, oldPath, control.MergeOptions{})
	if got.Plays != 3 || got.PlaysKnown != 0 {
		t.Fatalf("first merge: %+v, want 3 new plays", got)
	}
	if n := playCount(t, mine); n != 5 {
		t.Fatalf("after the merge there are %d plays, want 5", n)
	}

	again := merge(t, mine, oldPath, control.MergeOptions{})
	if again.Plays != 0 || again.PlaysKnown != 3 {
		t.Fatalf("second merge: %+v, want nothing new and 3 known", again)
	}
	if n := playCount(t, mine); n != 5 {
		t.Fatalf("a second merge changed the count to %d", n)
	}

	// Plays made over there since are picked up by a later merge.
	record(t, old, play("old-4", "t3", "Duman", "UC1", time.Minute, 100_000))
	later := merge(t, mine, oldPath, control.MergeOptions{})
	if later.Plays != 1 || later.PlaysKnown != 3 {
		t.Fatalf("later merge: %+v, want the one new play", later)
	}
}

// A play that is in both must be counted once: under the same identifier, or
// as the same track at the same instant under another.
func TestMergeDoesNotCountAPlayTwice(t *testing.T) {
	dir := t.TempDir()
	oldPath := filepath.Join(dir, "old.db")
	old := openAt(t, oldPath)
	mine := openAt(t, filepath.Join(dir, "mine.db"))

	shared := play("evt-shared", "t1", "Duman", "UC1", time.Hour, 200_000)
	sameMoment := play("evt-a", "t2", "Duman", "UC1", 2*time.Hour, 200_000)
	record(t, old, shared, sameMoment)
	sameMoment.EventUUID = "evt-b"
	record(t, mine, shared, sameMoment)

	got := merge(t, mine, oldPath, control.MergeOptions{})
	if got.Plays != 0 || got.PlaysKnown != 2 {
		t.Fatalf("merge: %+v, want both plays recognised", got)
	}
	if n := playCount(t, mine); n != 2 {
		t.Fatalf("%d plays after the merge, want 2", n)
	}
}

// The other app is running: its newest plays are still in the write-ahead
// log. They are read, and nothing in its folder is written.
func TestMergeReadsALiveDatabaseWithoutTouchingIt(t *testing.T) {
	dir := t.TempDir()
	oldDir := filepath.Join(dir, "old")
	if err := os.MkdirAll(oldDir, 0o700); err != nil {
		t.Fatal(err)
	}
	oldPath := filepath.Join(oldDir, "spotifier.db")
	old := openAt(t, oldPath)
	record(t, old, play("old-1", "t1", "Duman", "UC1", time.Hour, 200_000))
	// Still open, so the play has not been checkpointed into the main file.
	if info, err := os.Stat(oldPath + "-wal"); err != nil || info.Size() == 0 {
		t.Fatalf("expected a live write-ahead log: %v", err)
	}
	before := listing(t, oldDir)

	mine := openAt(t, filepath.Join(dir, "mine.db"))
	if got := merge(t, mine, oldPath, control.MergeOptions{}); got.Plays != 1 {
		t.Fatalf("merge: %+v, want the play from the log", got)
	}
	if after := listing(t, oldDir); after != before {
		t.Fatalf("the other profile changed:\nbefore %s\nafter  %s", before, after)
	}
	// And it is still usable by its owner.
	record(t, old, play("old-2", "t2", "Duman", "UC1", time.Minute, 200_000))
}

// listing is every file in dir with its size and time.
func listing(t *testing.T, dir string) string {
	t.Helper()
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatal(err)
	}
	out := ""
	for _, e := range entries {
		info, err := e.Info()
		if err != nil {
			t.Fatal(err)
		}
		out += e.Name() + " " + info.ModTime().String() + " " + time.Duration(info.Size()).String() + "; "
	}
	return out
}

// A database from a build before covers were kept, and before folders, pins
// and the resume point existed, still merges: the copy is migrated first.
func TestMergeTakesAnOlderSchema(t *testing.T) {
	dir := t.TempDir()
	oldPath := filepath.Join(dir, "old.db")
	raw, err := sql.Open("sqlite", oldPath)
	if err != nil {
		t.Fatal(err)
	}
	for _, stmt := range []string{
		`CREATE TABLE plays (
			id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL,
			event_uuid TEXT NOT NULL, track_id TEXT NOT NULL,
			title TEXT NOT NULL DEFAULT '', artist TEXT NOT NULL DEFAULT '',
			artist_id TEXT NOT NULL DEFAULT '', album TEXT NOT NULL DEFAULT '',
			album_id TEXT NOT NULL DEFAULT '', played_ms INTEGER NOT NULL DEFAULT 0,
			completed INTEGER NOT NULL DEFAULT 0, failed INTEGER NOT NULL DEFAULT 0,
			fail_reason TEXT NOT NULL DEFAULT '', origin TEXT NOT NULL DEFAULT '',
			played_at TIMESTAMP NOT NULL)`,
		// What an earlier build kept beside the plays; not this build's.
		`CREATE TABLE response_cache (key TEXT PRIMARY KEY, body BLOB)`,
		`INSERT INTO response_cache VALUES ('home', x'00')`,
	} {
		if _, err := raw.Exec(stmt); err != nil {
			t.Fatalf("%v\n%s", err, stmt)
		}
	}
	if _, err := raw.Exec(`INSERT INTO plays (user_id, event_uuid, track_id, title, artist, played_ms, played_at)
		VALUES (1, 'old-1', 't1', 'Bal', 'Song • Duman', 200000, ?)`, time.Now().UTC().Add(-time.Hour)); err != nil {
		t.Fatal(err)
	}
	if err := raw.Close(); err != nil {
		t.Fatal(err)
	}

	mine := openAt(t, filepath.Join(dir, "mine.db"))
	got := merge(t, mine, oldPath, control.MergeOptions{Resume: true})
	if got.Plays != 1 || got.Folders != 0 || got.Pins != 0 || got.Resume {
		t.Fatalf("merge: %+v, want one play and nothing else", got)
	}
	top, err := mine.TopArtists(context.Background(), control.DefaultUserID, control.Last(24*time.Hour), 5)
	if err != nil {
		t.Fatal(err)
	}
	// The copy went through the repairs too: a subtitle line is not an artist.
	if len(top) != 1 || top[0].Artist != "Duman" {
		t.Fatalf("top artists after the merge: %+v", top)
	}

	seen, err := control.Inspect(context.Background(), oldPath)
	if err != nil {
		t.Fatal(err)
	}
	if seen.Plays != 1 || seen.FirstPlay == "" || seen.FirstPlay != seen.LastPlay {
		t.Fatalf("inspect: %+v", seen)
	}
	// The original still has the shape it came with.
	raw, err = sql.Open("sqlite", oldPath)
	if err != nil {
		t.Fatal(err)
	}
	defer raw.Close()
	var tables int
	if err := raw.QueryRow(`SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('folders', 'response_cache')`).Scan(&tables); err != nil {
		t.Fatal(err)
	}
	if tables != 1 {
		t.Fatal("the original database was migrated; only its copy may be")
	}
}

// Folders come with their ids, a pin made in either profile holds, and an
// item filed here stays where it was put.
func TestMergeBringsFoldersAndPins(t *testing.T) {
	ctx := context.Background()
	dir := t.TempDir()
	oldPath := filepath.Join(dir, "old.db")
	old := openAt(t, oldPath)
	mine := openAt(t, filepath.Join(dir, "mine.db"))

	folder, err := old.CreateFolder(ctx, control.DefaultUserID, "Road trip")
	if err != nil {
		t.Fatal(err)
	}
	organise := func(s *control.Store, id, folderID string) {
		t.Helper()
		if err := s.SetPinned(ctx, control.DefaultUserID, "playlist", id, true); err != nil {
			t.Fatal(err)
		}
		if err := s.SetFolder(ctx, control.DefaultUserID, "playlist", id, folderID); err != nil {
			t.Fatal(err)
		}
	}
	organise(old, "PL1", folder)
	organise(old, "PL2", folder)
	// PL2 is already pinned here, in a folder of this profile's own.
	own, err := mine.CreateFolder(ctx, control.DefaultUserID, "Mine")
	if err != nil {
		t.Fatal(err)
	}
	organise(mine, "PL2", own)

	got := merge(t, mine, oldPath, control.MergeOptions{})
	if got.Folders != 1 || got.Pins != 1 || got.Filed != 1 {
		t.Fatalf("merge: %+v, want one folder, one pin and one filed item", got)
	}
	items := []domain.LibraryItem{{Kind: "playlist", ID: "PL1"}, {Kind: "playlist", ID: "PL2"}}
	if err := mine.Enrich(ctx, items); err != nil {
		t.Fatal(err)
	}
	if m := items[0]; !m.Pinned || m.FolderID != folder {
		t.Fatalf("PL1 came across as %+v", m)
	}
	if m := items[1]; !m.Pinned || m.FolderID != own {
		t.Fatalf("PL2 lost its place here: %+v", m)
	}
	folders, err := mine.Folders(ctx, control.DefaultUserID)
	if err != nil {
		t.Fatal(err)
	}
	if len(folders) != 2 {
		t.Fatalf("%d folders after the merge, want 2", len(folders))
	}

	again := merge(t, mine, oldPath, control.MergeOptions{})
	if again != (control.Merged{}) {
		t.Fatalf("second merge: %+v, want nothing new", again)
	}
}

// The queue left in the other profile is taken only when asked for, and only
// by a profile that has none of its own.
func TestMergeTakesTheResumePointOnlyIntoAnEmptyOne(t *testing.T) {
	ctx := context.Background()
	dir := t.TempDir()
	oldPath := filepath.Join(dir, "old.db")
	old := openAt(t, oldPath)
	if err := old.SaveResume(ctx, control.DefaultUserID, []byte(`{"theirs":true}`)); err != nil {
		t.Fatal(err)
	}

	mine := openAt(t, filepath.Join(dir, "mine.db"))
	if got := merge(t, mine, oldPath, control.MergeOptions{}); got.Resume {
		t.Fatal("the resume point was taken without being asked for")
	}
	if got := merge(t, mine, oldPath, control.MergeOptions{Resume: true}); !got.Resume {
		t.Fatal("the resume point was not taken into a profile with none")
	}
	if blob, _ := mine.Resume(ctx, control.DefaultUserID); string(blob) != `{"theirs":true}` {
		t.Fatalf("resume point after the merge: %s", blob)
	}

	busy := openAt(t, filepath.Join(dir, "busy.db"))
	if err := busy.SaveResume(ctx, control.DefaultUserID, []byte(`{"mine":true}`)); err != nil {
		t.Fatal(err)
	}
	if got := merge(t, busy, oldPath, control.MergeOptions{Resume: true}); got.Resume {
		t.Fatal("a queue this profile had was replaced")
	}
	if blob, _ := busy.Resume(ctx, control.DefaultUserID); string(blob) != `{"mine":true}` {
		t.Fatalf("resume point was overwritten: %s", blob)
	}
}

func TestInspectCountsWhatIsThere(t *testing.T) {
	ctx := context.Background()
	oldPath := filepath.Join(t.TempDir(), "old.db")
	old := openAt(t, oldPath)
	record(t, old,
		play("old-1", "t1", "Duman", "UC1", 48*time.Hour, 200_000),
		play("old-2", "t2", "Duman", "UC1", time.Hour, 100_000))
	if err := old.SetPinned(ctx, control.DefaultUserID, "album", "AL1", true); err != nil {
		t.Fatal(err)
	}

	got, err := control.Inspect(ctx, oldPath)
	if err != nil {
		t.Fatal(err)
	}
	if got.Plays != 2 || got.ListenedMs != 300_000 || got.Pins != 1 || got.Folders != 0 || got.Resume {
		t.Fatalf("inspect: %+v", got)
	}
	first, err1 := time.Parse(time.RFC3339, got.FirstPlay)
	last, err2 := time.Parse(time.RFC3339, got.LastPlay)
	if err1 != nil || err2 != nil || last.Sub(first) < 46*time.Hour {
		t.Fatalf("range %q to %q", got.FirstPlay, got.LastPlay)
	}

	if _, err := control.Inspect(ctx, filepath.Join(t.TempDir(), "absent.db")); err == nil {
		t.Fatal("a database that is not there was inspected")
	}
}

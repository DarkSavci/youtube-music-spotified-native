package audiocache

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
	"time"
)

// theirs is another profile's cache with these tracks in it, each of size
// bytes, the first used longest ago.
func theirs(t *testing.T, size int, ids ...string) string {
	t.Helper()
	dir := t.TempDir()
	other, err := New(dir, 1<<30)
	if err != nil {
		t.Fatal(err)
	}
	for _, id := range ids {
		fill(t, other, id, bytes.Repeat([]byte{id[len(id)-1]}, size), int64(size))
	}
	return dir
}

func TestImportCopiesWhatIsNotHereAndLeavesTheSourceAlone(t *testing.T) {
	src := theirs(t, 400, "track00001", "track00002")
	before, _ := os.ReadDir(src)
	c, err := New(t.TempDir(), 1<<20)
	if err != nil {
		t.Fatal(err)
	}
	fill(t, c, "track00002", bytes.Repeat([]byte("2"), 400), 400)

	got := c.Import(src, []string{"track00001", "track00002", "track00003", "../etc"})
	want := Imported{Copied: 1, Bytes: 400, Present: 1, Failed: 2}
	if got != want {
		t.Fatalf("import: %+v, want %+v", got, want)
	}
	f, m, err := c.Open("track00001")
	if err != nil || !m.Complete() || m.MimeType != "audio/webm" {
		t.Fatalf("the imported track: %+v %v", m, err)
	}
	f.Close()
	if bytes, tracks := c.Usage(); bytes != 800 || tracks != 2 {
		t.Fatalf("usage after the import: %d bytes in %d tracks", bytes, tracks)
	}
	if after, _ := os.ReadDir(src); len(after) != len(before) {
		t.Fatal("the other cache gained or lost files")
	}

	// Again: nothing more to bring, and the same holds after a restart.
	if again := c.Import(src, []string{"track00001", "track00002"}); again.Copied != 0 || again.Present != 2 {
		t.Fatalf("second import: %+v", again)
	}
	reopened, err := New(c.dir, 1<<20)
	if err != nil {
		t.Fatal(err)
	}
	if m, ok := reopened.Get("track00001"); !ok || !m.Complete() {
		t.Fatalf("the imported track did not survive a restart: %+v", m)
	}
}

func TestImportStopsAtTheCapAndEvictsNothing(t *testing.T) {
	src := theirs(t, 400, "track00001", "track00002")
	c, err := New(t.TempDir(), 1000)
	if err != nil {
		t.Fatal(err)
	}
	fill(t, c, "track00009", bytes.Repeat([]byte("9"), 400), 400)

	got := c.Import(src, []string{"track00001", "track00002"})
	if got.Copied != 1 || got.NoRoom != 1 {
		t.Fatalf("import: %+v, want one copied and one without room", got)
	}
	if _, ok := c.Get("track00009"); !ok {
		t.Fatal("a track that was here was evicted to make room")
	}
}

func TestImportCompletesATrackOnlyPartlyHere(t *testing.T) {
	src := theirs(t, 400, "track00001")
	c, err := New(t.TempDir(), 1<<20)
	if err != nil {
		t.Fatal(err)
	}
	fill(t, c, "track00001", bytes.Repeat([]byte("1"), 100), 400)

	if got := c.Import(src, []string{"track00001"}); got.Copied != 1 || got.Bytes != 400 {
		t.Fatalf("import: %+v", got)
	}
	if m, _ := c.Get("track00001"); !m.Complete() {
		t.Fatalf("still partial: %+v", m)
	}
	if bytes, _ := c.Usage(); bytes != 400 {
		t.Fatalf("the partial copy is still counted: %d bytes", bytes)
	}
}

func TestImportKeepsWhenATrackWasLastUsed(t *testing.T) {
	src := theirs(t, 400, "track00001")
	long := time.Now().Add(-90 * 24 * time.Hour).UTC().Truncate(time.Second)
	raw := []byte(`{"mimeType":"audio/webm","size":400,"have":400,"usedAt":"` + long.Format(time.RFC3339) + `"}`)
	if err := os.WriteFile(filepath.Join(src, "track00001.json"), raw, 0o600); err != nil {
		t.Fatal(err)
	}
	c, err := New(t.TempDir(), 1<<20)
	if err != nil {
		t.Fatal(err)
	}
	c.Import(src, []string{"track00001"})
	if m, _ := c.Get("track00001"); !m.UsedAt.Equal(long) {
		t.Fatalf("used at %v, want %v", m.UsedAt, long)
	}
}

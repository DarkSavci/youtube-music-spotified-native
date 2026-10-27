package audiocache

import (
	"bytes"
	"context"
	"io"
	"testing"
	"time"
)

func fill(t *testing.T, c *Cache, id string, data []byte, size int64) {
	t.Helper()
	w, ok, err := c.Writer(id, "audio/webm", size)
	if err != nil || !ok {
		t.Fatalf("writer %s: ok=%v err=%v", id, ok, err)
	}
	if _, err := w.Write(data); err != nil {
		t.Fatal(err)
	}
	if err := w.Close(); err != nil {
		t.Fatal(err)
	}
}

func TestPrefixThenRestSurvivesReopen(t *testing.T) {
	dir := t.TempDir()
	c, err := New(dir, 1<<20)
	if err != nil {
		t.Fatal(err)
	}
	whole := bytes.Repeat([]byte("abcdefgh"), 100)
	fill(t, c, "track00001", whole[:300], int64(len(whole)))
	if m, _ := c.Get("track00001"); m.Complete() || m.Have != 300 {
		t.Fatalf("after the prefix: %+v", m)
	}

	c, err = New(dir, 1<<20)
	if err != nil {
		t.Fatal(err)
	}
	w, ok, _ := c.Writer("track00001", "", 0)
	if !ok || w.Offset() != 300 {
		t.Fatalf("resume at %d, ok=%v", w.Offset(), ok)
	}
	_, _ = w.Write(whole[300:])
	_ = w.Close()

	f, m, err := c.Open("track00001")
	if err != nil || !m.Complete() {
		t.Fatalf("open: %+v %v", m, err)
	}
	defer f.Close()
	got, _ := io.ReadAll(f)
	if !bytes.Equal(got, whole) {
		t.Fatal("cached bytes differ from what was written")
	}
}

func TestOneWriterPerTrack(t *testing.T) {
	c, _ := New(t.TempDir(), 1<<20)
	w, ok, _ := c.Writer("track00002", "audio/webm", 10)
	if !ok {
		t.Fatal("first writer refused")
	}
	if _, ok, _ := c.Writer("track00002", "audio/webm", 10); ok {
		t.Fatal("a second writer was allowed")
	}
	_ = w.Close()
}

func TestEvictsLeastRecentlyUsed(t *testing.T) {
	c, _ := New(t.TempDir(), 250)
	fill(t, c, "oldtrack01", make([]byte, 100), 100)
	time.Sleep(5 * time.Millisecond)
	fill(t, c, "newtrack01", make([]byte, 100), 100)
	time.Sleep(5 * time.Millisecond)
	// Playing the older one makes it the one to keep.
	f, _, _ := c.Open("oldtrack01")
	f.Close()
	time.Sleep(5 * time.Millisecond)
	fill(t, c, "third00001", make([]byte, 100), 100)

	if _, ok := c.Get("newtrack01"); ok {
		t.Error("the least recently used track was kept")
	}
	if _, ok := c.Get("oldtrack01"); !ok {
		t.Error("a recently played track was evicted")
	}
}

func TestRejectsPathsAsIDs(t *testing.T) {
	c, _ := New(t.TempDir(), 1<<20)
	if _, _, err := c.Writer("../../evil", "", 0); err != ErrBadID {
		t.Fatalf("got %v", err)
	}
}

// A reader can follow a download in progress, byte by byte.
func TestWaitFollowsAWriter(t *testing.T) {
	c, _ := New(t.TempDir(), 1<<20)
	w, _, _ := c.Writer("growing001", "audio/webm", 20)
	go func() {
		for range 4 {
			time.Sleep(10 * time.Millisecond)
			_, _ = w.Write([]byte("12345"))
		}
		_ = w.Close()
	}()
	var have int64
	for {
		m, writing := c.Wait(context.Background(), "growing001", have)
		if m.Have > have {
			have = m.Have
		}
		if !writing {
			break
		}
	}
	if have != 20 {
		t.Fatalf("followed %d of 20 bytes", have)
	}
}

// A long download evicts older tracks as it goes, not only when it ends.
func TestEvictsWhileWriting(t *testing.T) {
	c, _ := New(t.TempDir(), 150)
	fill(t, c, "oldtrack02", make([]byte, 100), 100)
	w, _, _ := c.Writer("longtrack1", "audio/webm", 300)
	_, _ = w.Write(make([]byte, 100))
	if _, ok := c.Get("oldtrack02"); ok {
		t.Error("the cache went over its cap during a download")
	}
	_ = w.Close()
}

// A download is never evicted or cleared, and the mark outlives a restart.
func TestKeptTracksSurviveEvictionClearAndRestart(t *testing.T) {
	dir := t.TempDir()
	c, err := New(dir, 100)
	if err != nil {
		t.Fatal(err)
	}
	fill := func(id string, n int) {
		w, ok, err := c.Writer(id, "audio/webm", int64(n))
		if err != nil || !ok {
			t.Fatalf("writer %s: %v %v", id, ok, err)
		}
		if _, err := w.Write(make([]byte, n)); err != nil {
			t.Fatal(err)
		}
		w.Close()
	}
	if err := c.Keep("keepme1", true); err != nil {
		t.Fatal(err)
	}
	fill("keepme1", 80)
	fill("other01", 80) // over the cap: the other one goes, not the download
	if _, ok := c.Get("keepme1"); !ok {
		t.Fatal("a download was evicted")
	}
	c.Clear()
	if _, ok := c.Get("keepme1"); !ok {
		t.Fatal("a download was cleared")
	}

	again, err := New(dir, 100)
	if err != nil {
		t.Fatal(err)
	}
	kept := again.KeptTracks()
	if len(kept) != 1 || kept[0].ID != "keepme1" || !kept[0].Complete {
		t.Fatalf("after a restart: %+v", kept)
	}
	if err := again.Keep("keepme1", false); err != nil {
		t.Fatal(err)
	}
	if len(again.KeptTracks()) != 0 {
		t.Error("unkeeping left the mark")
	}
}

package audiocache

import (
	"encoding/json"
	"io"
	"os"
	"path/filepath"

	"spotifier/internal/readonly"
)

// Imported is what bringing tracks in from another cache came to.
type Imported struct {
	// Copied tracks, and the bytes they hold.
	Copied int   `json:"copied"`
	Bytes  int64 `json:"bytes"`
	// Present tracks were already here with as much audio, or more.
	Present int `json:"present"`
	// NoRoom tracks would have taken the cache past its cap.
	NoRoom int `json:"noRoom"`
	// Failed tracks could not be read: no record, no audio, or a bad id.
	Failed int `json:"failed"`
}

/*
Import copies the named tracks from another cache directory into this one.

The other directory is only read. A track is copied when this cache has less
of it than the other does, and only while it fits under the cap: nothing that
is already here is evicted to make room for audio that was never played here.
Each keeps the time it was last used over there, so what was listened to
longest ago is still the first to go.

It runs inside the cache, not beside it, because the cache keeps its books in
memory: a file put in the directory behind its back would not be known until
the next start, and could be overwritten by a download of the same track.
*/
func (c *Cache) Import(dir string, ids []string) Imported {
	var out Imported
	for _, id := range ids {
		if !validID.MatchString(id) {
			out.Failed++
			continue
		}
		raw, err := readonly.ReadFile(filepath.Join(dir, id+".json"))
		var theirs Meta
		if err != nil || json.Unmarshal(raw, &theirs) != nil || theirs.Have <= 0 {
			out.Failed++
			continue
		}

		c.mu.Lock()
		mine, known := c.metas[id]
		var have int64
		if known {
			have = mine.Have
		}
		switch {
		case c.writing[id] || have >= theirs.Have:
			c.mu.Unlock()
			out.Present++
			continue
		case c.used-have+theirs.Have > c.max:
			c.mu.Unlock()
			out.NoRoom++
			continue
		}
		// Held as a writer would hold it, so no download starts on the track
		// while its audio is being copied in.
		c.writing[id] = true
		c.mu.Unlock()

		part := c.dataPath(id) + ".part"
		copied, err := copyLeading(filepath.Join(dir, id+".audio"), part, theirs.Have)

		c.mu.Lock()
		delete(c.writing, id)
		if err == nil && copied > have {
			err = os.Rename(part, c.dataPath(id))
		} else if err == nil {
			err = os.ErrNotExist
		}
		if err != nil {
			_ = os.Remove(part)
			c.mu.Unlock()
			out.Failed++
			continue
		}
		theirs.Have = copied
		c.metas[id] = &theirs
		c.used += copied - have
		_ = c.saveLocked(id)
		c.notifyLocked()
		c.mu.Unlock()
		out.Copied++
		out.Bytes += copied
	}
	return out
}

// copyLeading copies the first n bytes of src to dst, or as many as src has:
// the record may claim bytes a crash never let land.
func copyLeading(src, dst string, n int64) (int64, error) {
	in, err := readonly.Open(src)
	if err != nil {
		return 0, err
	}
	defer in.Close()
	out, err := os.OpenFile(dst, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o600)
	if err != nil {
		return 0, err
	}
	copied, err := io.Copy(out, io.LimitReader(in, n))
	if cerr := out.Close(); err == nil {
		err = cerr
	}
	return copied, err
}

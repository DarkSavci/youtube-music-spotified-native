package control

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"

	"spotifier/internal/readonly"
)

/*
Bringing another profile's listening across.

The Electron app keeps the same database this core does, in its own folder.
Its plays, folders and pins are merged into this one rather than copied over
it: this profile may have plays of its own by then, and both sets are the
listener's history.

That folder is never written to, and its database is never opened where it
lies. SQLite cannot read a database in write-ahead mode without taking locks
in its -shm file, and the only way round that (immutable=1) ignores the
write-ahead log, which is where the newest plays are while the other app
runs. So the database and its log are copied to a temporary folder, and the
copy is what gets opened.
*/

// OldData is what a database from another profile holds, for showing before
// anything is brought across.
type OldData struct {
	Plays int64 `json:"plays"`
	// FirstPlay and LastPlay are RFC 3339, empty when there are no plays.
	FirstPlay  string `json:"firstPlay"`
	LastPlay   string `json:"lastPlay"`
	ListenedMs int64  `json:"listenedMs"`
	Pins       int64  `json:"pins"`
	Folders    int64  `json:"folders"`
	// Resume is whether a queue was left to come back to.
	Resume bool `json:"resume"`
}

// Merged is what a merge added. Everything is a count of what was new here:
// a second merge of the same database reports zeroes.
type Merged struct {
	Plays int64 `json:"plays"`
	// PlaysKnown were already here, from an earlier merge.
	PlaysKnown int64 `json:"playsKnown"`
	Folders    int64 `json:"folders"`
	Pins       int64 `json:"pins"`
	// Filed are library items put in a folder they were not in here.
	Filed int64 `json:"filed"`
	// Resume is whether the queue left in the other profile was taken.
	Resume bool `json:"resume"`
}

// MergeOptions says what besides the history a merge takes.
type MergeOptions struct {
	// Resume takes the queue left in the other profile when this one has
	// none. Only for a database no core is running on: a running core writes
	// its own state over the row within seconds.
	Resume bool
}

// errUnsteady is a database that kept changing under every attempt to copy it.
var errUnsteady = errors.New("control: the database was being rewritten; try again in a moment")

// walHeaderSize is the part of a write-ahead log that names its generation:
// the salts change whenever the log is started over.
const walHeaderSize = 32

/*
snapshot copies a database and its write-ahead log into dir, and returns the
copy's path.

The pair is consistent as long as the log was not started over between the
two copies: the database is copied first, so whatever a checkpoint wrote into
it meanwhile is still in the log copied after it, and the log wins. Frames
appended during the copy are either whole, and read, or torn, and ignored by
their checksums. A log that was started over is detected by its header, and
the copy is made again.
*/
func snapshot(src, dir string) (string, error) {
	dst := filepath.Join(dir, "old.db")
	for attempt := 0; attempt < 6; attempt++ {
		if attempt > 0 {
			time.Sleep(150 * time.Millisecond)
		}
		before, err := generation(src)
		if err != nil {
			return "", err
		}
		if err := copyFile(src, dst); err != nil {
			return "", fmt.Errorf("control: copy database: %w", err)
		}
		if err := copyFile(src+"-wal", dst+"-wal"); err != nil {
			if !errors.Is(err, os.ErrNotExist) {
				return "", fmt.Errorf("control: copy write-ahead log: %w", err)
			}
			// No log: everything is in the database file.
			_ = os.Remove(dst + "-wal")
		}
		after, err := generation(src)
		if err != nil {
			return "", err
		}
		if bytes.Equal(before, after) {
			return dst, nil
		}
	}
	return "", errUnsteady
}

// generation identifies the state a copy must not straddle: the log's header,
// or, with no log, the database file's size and time.
func generation(src string) ([]byte, error) {
	info, err := os.Stat(src)
	if err != nil {
		return nil, fmt.Errorf("control: %w", err)
	}
	wal, err := readonly.Open(src + "-wal")
	if err != nil {
		return fmt.Appendf(nil, "%d %d", info.Size(), info.ModTime().UnixNano()), nil
	}
	defer wal.Close()
	header := make([]byte, walHeaderSize)
	n, _ := io.ReadFull(wal, header)
	if n < walHeaderSize {
		// An empty log holds nothing; the database file is the whole state.
		return fmt.Appendf(nil, "%d %d", info.Size(), info.ModTime().UnixNano()), nil
	}
	return header, nil
}

// copyFile copies src, opened so that its owner is not held up, to dst.
func copyFile(src, dst string) error {
	in, err := readonly.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.OpenFile(dst, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o600)
	if err != nil {
		return err
	}
	if _, err := io.Copy(out, in); err != nil {
		_ = out.Close()
		return err
	}
	return out.Close()
}

/*
prepared copies the database at path aside and brings the copy up to this
build's schema, by the same migrations a start-up runs. A database written by
an older build then has every table and column the merge names, and one from
a newer build only has more than is read. cleanup removes the copy.
*/
func prepared(ctx context.Context, path string) (copy string, cleanup func(), err error) {
	dir, err := os.MkdirTemp("", "spotified-migrate-")
	if err != nil {
		return "", nil, fmt.Errorf("control: %w", err)
	}
	cleanup = func() { _ = os.RemoveAll(dir) }
	copy, err = snapshot(path, dir)
	if err != nil {
		cleanup()
		return "", nil, err
	}
	old, err := Open(ctx, copy)
	if err != nil {
		cleanup()
		return "", nil, fmt.Errorf("control: the copied database could not be read: %w", err)
	}
	// A copy that straddled a rewrite in a way the header did not show is
	// caught here rather than merged.
	var verdict string
	if err := old.db.QueryRowContext(ctx, `PRAGMA quick_check(1)`).Scan(&verdict); err != nil || verdict != "ok" {
		old.Close()
		cleanup()
		return "", nil, fmt.Errorf("control: the copied database is damaged (%s)", verdict)
	}
	if err := old.Close(); err != nil {
		cleanup()
		return "", nil, fmt.Errorf("control: %w", err)
	}
	return copy, cleanup, nil
}

// Inspect reports what the database at path holds, without changing it.
func Inspect(ctx context.Context, path string) (OldData, error) {
	var out OldData
	copy, cleanup, err := prepared(ctx, path)
	if err != nil {
		return out, err
	}
	defer cleanup()
	old, err := Open(ctx, copy)
	if err != nil {
		return out, err
	}
	defer old.Close()

	var first, last sqlTime
	if err := old.db.QueryRowContext(ctx, `
		SELECT COUNT(*), MIN(played_at), MAX(played_at), COALESCE(SUM(played_ms), 0)
		FROM plays WHERE user_id = ?`, DefaultUserID).
		Scan(&out.Plays, &first, &last, &out.ListenedMs); err != nil {
		return out, fmt.Errorf("control: inspect plays: %w", err)
	}
	if first.Valid {
		out.FirstPlay = first.Time.Format(time.RFC3339)
	}
	if last.Valid {
		out.LastPlay = last.Time.Format(time.RFC3339)
	}
	if err := old.db.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM library_meta WHERE user_id = ? AND pinned = 1`, DefaultUserID).
		Scan(&out.Pins); err != nil {
		return out, fmt.Errorf("control: inspect pins: %w", err)
	}
	if err := old.db.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM folders WHERE user_id = ?`, DefaultUserID).
		Scan(&out.Folders); err != nil {
		return out, fmt.Errorf("control: inspect folders: %w", err)
	}
	blob, err := old.Resume(ctx, DefaultUserID)
	if err != nil {
		return out, err
	}
	out.Resume = len(blob) > 0
	return out, nil
}

/*
MergeFrom adds what the database at path holds to this one.

Plays are told apart by the identifier their client gave them, so merging the
same database again adds nothing; and a play of the same track at the same
instant is taken to be the same play even under another identifier, which is
what a database that was once copied by hand would hold. Folders keep their
ids. A library item is pinned if it was pinned in either, keeps the folder it
has here or takes the other's, and keeps the earlier first-seen date.

All of it is one transaction: a merge that fails leaves this database as it
was.
*/
func (s *Store) MergeFrom(ctx context.Context, path string, opts MergeOptions) (Merged, error) {
	var out Merged
	copy, cleanup, err := prepared(ctx, path)
	if err != nil {
		return out, err
	}
	defer cleanup()

	// ATTACH belongs to a connection, so everything runs on one.
	conn, err := s.db.Conn(ctx)
	if err != nil {
		return out, fmt.Errorf("control: merge: %w", err)
	}
	defer conn.Close()
	if _, err := conn.ExecContext(ctx, `ATTACH DATABASE ? AS old`, copy); err != nil {
		return out, fmt.Errorf("control: attach: %w", err)
	}
	// Detached whatever happens, or the copy could not be deleted and the
	// connection would carry it into every later query.
	defer func() {
		_, _ = conn.ExecContext(context.WithoutCancel(ctx), `DETACH DATABASE old`)
	}()

	tx, err := conn.BeginTx(ctx, nil)
	if err != nil {
		return out, fmt.Errorf("control: merge: %w", err)
	}
	defer tx.Rollback()
	if err := mergeAttached(ctx, tx, opts, &out); err != nil {
		return Merged{}, err
	}
	if err := tx.Commit(); err != nil {
		return Merged{}, fmt.Errorf("control: merge: %w", err)
	}
	return out, nil
}

func mergeAttached(ctx context.Context, tx *sql.Tx, opts MergeOptions, out *Merged) error {
	var total int64
	if err := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM old.plays`).Scan(&total); err != nil {
		return fmt.Errorf("control: merge plays: %w", err)
	}
	added, err := tx.ExecContext(ctx, `
		INSERT INTO plays (user_id, event_uuid, track_id, title, artist, artist_id,
		                   album, album_id, played_ms, completed, failed, fail_reason,
		                   origin, played_at, artwork)
		SELECT o.user_id, o.event_uuid, o.track_id, o.title, o.artist, o.artist_id,
		       o.album, o.album_id, o.played_ms, o.completed, o.failed, o.fail_reason,
		       o.origin, o.played_at, o.artwork
		FROM old.plays o
		WHERE NOT EXISTS (
			SELECT 1 FROM plays p
			WHERE p.user_id = o.user_id AND p.track_id = o.track_id AND p.played_at = o.played_at)
		ORDER BY o.id
		ON CONFLICT(user_id, event_uuid) DO NOTHING`)
	if err != nil {
		return fmt.Errorf("control: merge plays: %w", err)
	}
	out.Plays, _ = added.RowsAffected()
	out.PlaysKnown = total - out.Plays

	// "WHERE true" keeps SQLite from reading ON CONFLICT as a join clause.
	folders, err := tx.ExecContext(ctx, `
		INSERT INTO folders (id, user_id, name, parent_id, created_at)
		SELECT id, user_id, name, parent_id, created_at FROM old.folders WHERE true
		ON CONFLICT(id) DO NOTHING`)
	if err != nil {
		return fmt.Errorf("control: merge folders: %w", err)
	}
	out.Folders, _ = folders.RowsAffected()

	// Counted before the rows are merged: afterwards there is no telling
	// which pins were here already.
	const unmatched = `NOT EXISTS (
		SELECT 1 FROM library_meta m
		WHERE m.user_id = o.user_id AND m.item_kind = o.item_kind AND m.item_id = o.item_id AND `
	if err := tx.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM old.library_meta o WHERE o.pinned = 1 AND `+unmatched+`m.pinned = 1)`).
		Scan(&out.Pins); err != nil {
		return fmt.Errorf("control: merge pins: %w", err)
	}
	if err := tx.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM old.library_meta o WHERE o.folder_id <> '' AND `+unmatched+`m.folder_id <> '')`).
		Scan(&out.Filed); err != nil {
		return fmt.Errorf("control: merge filed items: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO library_meta (user_id, item_kind, item_id, folder_id, pinned, first_seen_at)
		SELECT user_id, item_kind, item_id, folder_id, pinned, first_seen_at
		FROM old.library_meta WHERE true
		ON CONFLICT(user_id, item_kind, item_id) DO UPDATE SET
			pinned = MAX(library_meta.pinned, excluded.pinned),
			folder_id = CASE WHEN library_meta.folder_id = '' THEN excluded.folder_id ELSE library_meta.folder_id END,
			first_seen_at = MIN(library_meta.first_seen_at, excluded.first_seen_at)`); err != nil {
		return fmt.Errorf("control: merge library: %w", err)
	}

	if opts.Resume {
		resume, err := tx.ExecContext(ctx, `
			INSERT INTO resume_state (user_id, snapshot, saved_at)
			SELECT user_id, snapshot, saved_at FROM old.resume_state WHERE true
			ON CONFLICT(user_id) DO NOTHING`)
		if err != nil {
			return fmt.Errorf("control: merge resume: %w", err)
		}
		n, _ := resume.RowsAffected()
		out.Resume = n > 0
	}
	return nil
}

package readonly

import (
	"errors"
	"io"
	"os"
	"path/filepath"
	"testing"
)

// The owner of a file being read here can still rename it away and delete it,
// which os.Open would not allow on Windows.
func TestAFileBeingReadCanStillBeReplacedAndDeletedByItsOwner(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "theirs.db-wal")
	if err := os.WriteFile(path, []byte("frames"), 0o600); err != nil {
		t.Fatal(err)
	}
	f, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	if err := os.Rename(path, path+".old"); err != nil {
		t.Fatalf("the owner could not move the file: %v", err)
	}
	if err := os.Remove(path + ".old"); err != nil {
		t.Fatalf("the owner could not delete the file: %v", err)
	}
	// What was opened is still readable to its end.
	got, err := io.ReadAll(f)
	if err != nil || string(got) != "frames" {
		t.Fatalf("read %q, %v", got, err)
	}
}

func TestAMissingFileIsNotFound(t *testing.T) {
	_, err := Open(filepath.Join(t.TempDir(), "absent"))
	if !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("got %v, want not-exist", err)
	}
	if _, err := ReadFile(filepath.Join(t.TempDir(), "absent")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("got %v, want not-exist", err)
	}
}

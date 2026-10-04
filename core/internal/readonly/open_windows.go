//go:build windows

package readonly

import (
	"os"
	"syscall"
)

// Open opens a file for reading and lets everyone else do as they please
// with it meanwhile, deleting it included.
//
// os.Open on Windows shares reading and writing but not deleting, so while
// this process held a file its owner could not remove or replace it: SQLite
// could not drop its write-ahead log, and a song cache could not evict a
// track. A reader of somebody else's files must not get in their way.
func Open(path string) (*os.File, error) {
	name, err := syscall.UTF16PtrFromString(path)
	if err != nil {
		return nil, &os.PathError{Op: "open", Path: path, Err: err}
	}
	handle, err := syscall.CreateFile(name, syscall.GENERIC_READ,
		syscall.FILE_SHARE_READ|syscall.FILE_SHARE_WRITE|syscall.FILE_SHARE_DELETE,
		nil, syscall.OPEN_EXISTING, syscall.FILE_ATTRIBUTE_NORMAL, 0)
	if err != nil {
		return nil, &os.PathError{Op: "open", Path: path, Err: err}
	}
	return os.NewFile(uintptr(handle), path), nil
}

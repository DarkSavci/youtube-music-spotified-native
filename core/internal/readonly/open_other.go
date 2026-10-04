//go:build !windows

package readonly

import "os"

// Open opens a file for reading. Elsewhere than Windows an open file never
// stops its owner removing or replacing it, so this is os.Open.
func Open(path string) (*os.File, error) { return os.Open(path) }

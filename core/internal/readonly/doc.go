// Package readonly reads files that belong to another program, which may be
// running, without holding them in any way that program could notice.
package readonly

import "io"

// ReadFile reads the whole of a file opened with Open.
func ReadFile(path string) ([]byte, error) {
	f, err := Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	return io.ReadAll(f)
}

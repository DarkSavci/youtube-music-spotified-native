//go:build android || sqlitewasm

package control

/*
Android's SQLite: SQLite's C built to WebAssembly and run by wazero.

modernc's translation calls the kernel directly, and on x86_64 it asks for
open, stat, lstat and other calls Android's seccomp filter does not allow,
so the core was killed opening its database on any x86_64 Android, the
emulator included. This one does its I/O through Go's os package, which
Android allows on every architecture. The sqlitewasm build tag runs the
desktop's tests against it.
*/
import (
	_ "github.com/ncruces/go-sqlite3/driver"
	_ "github.com/ncruces/go-sqlite3/embed"
)

const driverName = "sqlite3"

// dsn is a database path with its pragmas, as a file: URI, which is how
// this driver takes parameters.
func dsn(path, pragmas string) string { return "file:" + path + "?" + pragmas }

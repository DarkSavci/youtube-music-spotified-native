//go:build !android && !sqlitewasm

package control

// The desktop's SQLite: modernc's, SQLite's C translated to Go.
import _ "modernc.org/sqlite"

const driverName = "sqlite"

// dsn is a database path with its pragmas, as modernc's driver takes them.
func dsn(path, pragmas string) string { return path + "?" + pragmas }

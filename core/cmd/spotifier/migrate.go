package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"

	"spotifier/internal/control"
)

/*
migrate is the core run for one job instead of as a server: reading, or
merging in, the database of another profile.

It is how the app brings history into the database of an account that is not
the one in use. No core is running on such a database, so this process is its
only writer for as long as the merge takes; the account in use goes through
the running core's own route instead. The answer is one line of JSON on
standard output, and the exit status says whether there is one.
*/
func migrate(inspect, from, db string) int {
	ctx := context.Background()
	var (
		answer any
		err    error
	)
	if inspect != "" {
		answer, err = control.Inspect(ctx, inspect)
	} else {
		var store *control.Store
		if store, err = control.Open(ctx, db); err == nil {
			// Nothing is running on this database, so the queue left in the
			// other profile can be taken when this one has none.
			answer, err = store.MergeFrom(ctx, from, control.MergeOptions{Resume: true})
			if cerr := store.Close(); err == nil {
				err = cerr
			}
		}
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	if err := json.NewEncoder(os.Stdout).Encode(answer); err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	return 0
}

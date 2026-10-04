package api

import (
	"encoding/json"
	"net/http"

	"spotifier/internal/control"
)

/*
Bringing a profile of the Electron app across.

The app finds that profile and decides what to bring; these two routes do the
parts that only this process can do safely. The database is this process's
while it runs, so the merge happens on its own connection; and the song cache
keeps its books in memory, so audio put in its folder by anyone else would not
be known to it.

Both name paths on this computer, which no web page has any business doing.
A browser always says where a cross-origin POST comes from, and the app never
does, so a request carrying an Origin is refused.
*/

func (s *Server) fromApp(w http.ResponseWriter, r *http.Request) bool {
	if r.Header.Get("Origin") != "" {
		s.write(w, http.StatusForbidden, apiError{Error: "only the app may ask for this"})
		return false
	}
	return true
}

// handleMigrateHistory merges another profile's plays, folders and pins.
func (s *Server) handleMigrateHistory(w http.ResponseWriter, r *http.Request) {
	if !s.fromApp(w, r) || !s.requireControl(w) {
		return
	}
	var body struct {
		Path string `json:"path"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil || body.Path == "" {
		s.write(w, http.StatusBadRequest, apiError{Error: "invalid body"})
		return
	}
	// The queue left over there is not taken: this process writes its own
	// over the row every few seconds.
	merged, err := s.deps.Control.MergeFrom(r.Context(), body.Path, control.MergeOptions{})
	if err != nil {
		s.deps.Log.Warn("migrate: history", "err", err)
		s.write(w, http.StatusUnprocessableEntity, apiError{Error: err.Error()})
		return
	}
	s.deps.Log.Info("migrate: history merged", "plays", merged.Plays, "known", merged.PlaysKnown,
		"folders", merged.Folders, "pins", merged.Pins)
	s.write(w, http.StatusOK, merged)
}

// handleMigrateCache copies tracks from another profile's song cache.
func (s *Server) handleMigrateCache(w http.ResponseWriter, r *http.Request) {
	if !s.fromApp(w, r) {
		return
	}
	if s.deps.Audio == nil {
		s.write(w, http.StatusServiceUnavailable, apiError{Error: "cache unavailable"})
		return
	}
	var body struct {
		Dir string   `json:"dir"`
		IDs []string `json:"ids"`
		// MaxMB sets the cache's cap first, when the app is bringing a larger
		// one across with the songs: the setting itself arrives afterwards.
		MaxMB int64 `json:"maxMB"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil || body.Dir == "" {
		s.write(w, http.StatusBadRequest, apiError{Error: "invalid body"})
		return
	}
	if body.MaxMB > 0 {
		s.deps.Audio.SetMax(body.MaxMB << 20)
	}
	s.write(w, http.StatusOK, s.deps.Audio.Import(body.Dir, body.IDs))
}

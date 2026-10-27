package api_test

import (
	"encoding/json"
	"net/http"
	"testing"

	"spotifier/internal/api"
	"spotifier/internal/audiocache"
)

// A download is listed once asked for, and gone once let go.
func TestDownloadsAreKeptAndListed(t *testing.T) {
	cache, err := audiocache.New(t.TempDir(), 1<<20)
	if err != nil {
		t.Fatal(err)
	}
	s := serverWith(api.Deps{Audio: cache})

	if rec := do(t, s, http.MethodPost, "/v1/downloads/abcdefghijk"); rec.Code != http.StatusNoContent {
		t.Fatalf("keep answered %d", rec.Code)
	}
	var kept []audiocache.Kept
	rec := do(t, s, http.MethodGet, "/v1/downloads")
	if err := json.Unmarshal(rec.Body.Bytes(), &kept); err != nil || len(kept) != 1 || kept[0].ID != "abcdefghijk" {
		t.Fatalf("listed %s (%v)", rec.Body, err)
	}
	if rec := do(t, s, http.MethodDelete, "/v1/downloads/abcdefghijk"); rec.Code != http.StatusNoContent {
		t.Fatalf("let go answered %d", rec.Code)
	}
	rec = do(t, s, http.MethodGet, "/v1/downloads")
	if err := json.Unmarshal(rec.Body.Bytes(), &kept); err != nil || len(kept) != 0 {
		t.Fatalf("still listed: %s", rec.Body)
	}
	if rec := do(t, s, http.MethodPost, "/v1/downloads/..%2Fx"); rec.Code != http.StatusBadRequest {
		t.Errorf("a bad id answered %d", rec.Code)
	}
}

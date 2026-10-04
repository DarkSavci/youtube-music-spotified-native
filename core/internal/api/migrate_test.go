package api_test

import (
	"bytes"
	"context"
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"
	"time"

	"spotifier/internal/api"
	"spotifier/internal/audiocache"
	"spotifier/internal/control"
)

func postJSON(t *testing.T, url string, body any, origin string) (*http.Response, []byte) {
	t.Helper()
	raw, _ := json.Marshal(body)
	req, err := http.NewRequest(http.MethodPost, url, bytes.NewReader(raw))
	if err != nil {
		t.Fatal(err)
	}
	req.Header.Set("Content-Type", "application/json")
	if origin != "" {
		req.Header.Set("Origin", origin)
	}
	res, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	defer res.Body.Close()
	out, _ := io.ReadAll(res.Body)
	return res, out
}

// The running core merges another profile's history into the database it
// owns, once; and a web page, which always says where it is from, may not ask.
func TestMigrateRoutesMergeOnceAndRefuseWebPages(t *testing.T) {
	ctx := context.Background()
	dir := t.TempDir()
	oldPath := filepath.Join(dir, "old.db")
	old, err := control.Open(ctx, oldPath)
	if err != nil {
		t.Fatal(err)
	}
	defer old.Close()
	if err := old.RecordPlays(ctx, control.DefaultUserID, []control.Play{
		{EventUUID: "old-1", TrackID: "t1", Title: "Bal", Artist: "Duman", PlayedMs: 200_000, PlayedAt: time.Now().Add(-time.Hour)},
	}); err != nil {
		t.Fatal(err)
	}
	mine, err := control.Open(ctx, filepath.Join(dir, "mine.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer mine.Close()
	cache, err := audiocache.New(filepath.Join(dir, "cache"), 1<<20)
	if err != nil {
		t.Fatal(err)
	}
	srv := httptest.NewServer(api.New(api.Deps{
		Control: mine, Audio: cache,
		Log: slog.New(slog.NewTextHandler(io.Discard, nil)),
	}))
	defer srv.Close()

	for _, route := range []string{"/v1/migrate/history", "/v1/migrate/cache"} {
		res, _ := postJSON(t, srv.URL+route, map[string]any{"path": oldPath, "dir": dir}, "https://example.com")
		if res.StatusCode != http.StatusForbidden {
			t.Fatalf("%s answered a web page with %d", route, res.StatusCode)
		}
	}

	for round, want := range []control.Merged{{Plays: 1}, {PlaysKnown: 1}} {
		res, raw := postJSON(t, srv.URL+"/v1/migrate/history", map[string]string{"path": oldPath}, "")
		var got control.Merged
		if res.StatusCode != http.StatusOK || json.Unmarshal(raw, &got) != nil || got != want {
			t.Fatalf("round %d: %d %s, want %+v", round, res.StatusCode, raw, want)
		}
	}

	res, raw := postJSON(t, srv.URL+"/v1/migrate/history", map[string]string{"path": filepath.Join(dir, "absent.db")}, "")
	if res.StatusCode != http.StatusUnprocessableEntity {
		t.Fatalf("a database that is not there: %d %s", res.StatusCode, raw)
	}

	res, raw = postJSON(t, srv.URL+"/v1/migrate/cache", map[string]any{"dir": dir, "ids": []string{"track00001"}}, "")
	var imported audiocache.Imported
	if res.StatusCode != http.StatusOK || json.Unmarshal(raw, &imported) != nil || imported.Failed != 1 {
		t.Fatalf("cache import: %d %s", res.StatusCode, raw)
	}
}

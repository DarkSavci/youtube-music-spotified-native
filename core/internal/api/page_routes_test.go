package api_test

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"

	"spotifier/internal/account"
	"spotifier/internal/api"
	"spotifier/internal/clock"
	"spotifier/internal/domain"
	"spotifier/internal/identity"
	"spotifier/internal/innertube"
)

// editingIdentity records the writes behind the playlist and library routes.
type editingIdentity struct {
	likedIdentity
	mu     sync.Mutex
	titles []string
	saved  []string
}

func (f *editingIdentity) EditPlaylist(_ context.Context, id string, title, _ *string) error {
	f.mu.Lock()
	defer f.mu.Unlock()
	if title != nil {
		f.titles = append(f.titles, id+"="+*title)
	}
	return nil
}

func (f *editingIdentity) SetPlaylistSaved(_ context.Context, id string, saved bool) error {
	f.note("playlist", id, saved)
	return nil
}

func (f *editingIdentity) SetAlbumSaved(_ context.Context, id string, saved bool) error {
	f.note("album", id, saved)
	return nil
}

func (f *editingIdentity) note(kind, id string, saved bool) {
	f.mu.Lock()
	defer f.mu.Unlock()
	word := "out"
	if saved {
		word = "in"
	}
	f.saved = append(f.saved, kind+":"+id+":"+word)
}

func send(s http.Handler, method, path, body string) *httptest.ResponseRecorder {
	r := httptest.NewRequest(method, path, strings.NewReader(body))
	w := httptest.NewRecorder()
	s.ServeHTTP(w, r)
	return w
}

func TestAPlaylistCanBeRenamed(t *testing.T) {
	id := &editingIdentity{}
	s := serverWith(api.Deps{Account: signedInAs(id)})
	if w := send(s, http.MethodPost, "/v1/me/playlists/PL1", `{"title":"Road"}`); w.Code != http.StatusNoContent {
		t.Fatalf("status %d: %s", w.Code, w.Body)
	}
	if len(id.titles) != 1 || id.titles[0] != "PL1=Road" {
		t.Fatalf("titles %v", id.titles)
	}
	if w := send(s, http.MethodPost, "/v1/me/playlists/PL1", `{"title":""}`); w.Code != http.StatusBadRequest {
		t.Fatalf("an empty name got %d", w.Code)
	}
}

func TestAlbumsAndPlaylistsCanBeSaved(t *testing.T) {
	id := &editingIdentity{}
	s := serverWith(api.Deps{Account: signedInAs(id)})
	for _, body := range []string{
		`{"kind":"album","itemId":"MPRE1","saved":true}`,
		`{"kind":"playlist","itemId":"PL2","saved":false}`,
	} {
		if w := send(s, http.MethodPost, "/v1/me/library/saved", body); w.Code != http.StatusNoContent {
			t.Fatalf("%s: status %d: %s", body, w.Code, w.Body)
		}
	}
	got, _ := json.Marshal(id.saved)
	if string(got) != `["album:MPRE1:in","playlist:PL2:out"]` {
		t.Fatalf("saved %s", got)
	}
	if w := send(s, http.MethodPost, "/v1/me/library/saved", `{"kind":"artist","itemId":"UC"}`); w.Code != http.StatusBadRequest {
		t.Fatalf("an artist got %d", w.Code)
	}
}

// A pull to refresh must reach YouTube, or it would show the kept page again.
func TestNoCacheReadsAgainAndAKeptCopyStillCoversAFailure(t *testing.T) {
	cat := &countingCatalog{}
	s := serverWith(api.Deps{Catalog: cat, Responses: kept(clock.NewManual())})
	first := do(t, s, http.MethodGet, "/v1/albums/MPREb_x").Body.String()

	fresh := func() *httptest.ResponseRecorder {
		r := httptest.NewRequest(http.MethodGet, "/v1/albums/MPREb_x", nil)
		r.Header.Set("Cache-Control", "no-cache")
		w := httptest.NewRecorder()
		s.ServeHTTP(w, r)
		return w
	}
	fresh()
	if n, _ := cat.count(); n != 2 {
		t.Fatalf("album read upstream %d times, want 2", n)
	}

	cat.mu.Lock()
	cat.fail = errors.New("innertube browse: connection lost")
	cat.mu.Unlock()
	if w := fresh(); w.Code != 200 || w.Body.String() != first {
		t.Fatalf("got %d %s, want the kept album", w.Code, w.Body)
	}
}

// missingCatalog is a country where YouTube Music has no such page.
type missingCatalog struct{ emptyCatalog }

func (missingCatalog) Browse(context.Context, string, string) (domain.BrowsePage, error) {
	return domain.BrowsePage{}, &innertube.HTTPError{Status: 404, Endpoint: "browse"}
}

func TestAPageYouTubeDoesNotHaveIsNotAFailure(t *testing.T) {
	s := serverWith(api.Deps{Catalog: missingCatalog{}})
	if w := do(t, s, http.MethodGet, "/v1/browse/FEmusic_non_music_audio"); w.Code != http.StatusNotFound {
		t.Fatalf("status %d: %s", w.Code, w.Body)
	}
}

func signedInAs(id identity.Identity) *account.Store {
	return account.Static(account.State{Identity: id})
}

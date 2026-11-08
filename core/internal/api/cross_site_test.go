package api

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

// A web page open in a browser can send requests to the loopback address.
// The routes that steer playback, sign the account in or out, or bring
// data in must refuse it, and nothing the core says may be readable to it.
func TestWebPagesCannotSteerTheSessionOrTheAccount(t *testing.T) {
	s := New(Deps{})
	routes := []struct{ method, path string }{
		{http.MethodPost, "/v1/session/command"},
		{http.MethodPost, "/v1/session/register"},
		{http.MethodPost, "/v1/session/settings"},
		{http.MethodPost, "/v1/session/radio"},
		{http.MethodGet, "/v1/session"},
		{http.MethodGet, "/v1/session/events"},
		{http.MethodPost, "/v1/auth/reload"},
		{http.MethodPost, "/v1/auth/sign-out"},
		{http.MethodPost, "/v1/migrate/history"},
		{http.MethodPost, "/v1/migrate/cache"},
		{http.MethodDelete, "/v1/cache"},
		{http.MethodPost, "/v1/downloads/abcdefghijk"},
	}
	for _, route := range routes {
		r := httptest.NewRequest(route.method, route.path, strings.NewReader(`{}`))
		r.Header.Set("Origin", "https://attacker.test")
		r.Header.Set("Sec-Fetch-Site", "cross-site")
		w := httptest.NewRecorder()
		s.ServeHTTP(w, r)
		if w.Code != http.StatusForbidden {
			t.Errorf("%s %s from a web page: status %d, want 403", route.method, route.path, w.Code)
		}
	}
}

func TestTheAppsOwnRequestsStillReachTheSessionRoutes(t *testing.T) {
	s := New(Deps{})
	// The apps send neither Origin nor Sec-Fetch headers.
	r := httptest.NewRequest(http.MethodPost, "/v1/auth/reload", nil)
	w := httptest.NewRecorder()
	s.ServeHTTP(w, r)
	if w.Code == http.StatusForbidden {
		t.Fatalf("the app's own request was refused")
	}
}

func TestNothingTheCoreSaysIsReadableToAWebPage(t *testing.T) {
	s := New(Deps{})
	for _, method := range []string{http.MethodGet, http.MethodOptions} {
		r := httptest.NewRequest(method, "/v1/health", nil)
		r.Header.Set("Origin", "https://attacker.test")
		w := httptest.NewRecorder()
		s.ServeHTTP(w, r)
		if got := w.Header().Get("Access-Control-Allow-Origin"); got != "" {
			t.Errorf("%s: Access-Control-Allow-Origin %q lets any page read the answer", method, got)
		}
	}
}

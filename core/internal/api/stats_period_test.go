package api

import (
	"net/http/httptest"
	"testing"
	"time"
)

func TestPeriodFromNamesAnExactSpan(t *testing.T) {
	r := httptest.NewRequest("GET", "/v1/me/stats/summary?days=365&from=1735689600&to=1767225600", nil)
	p := periodFrom(r)
	if !p.From.Equal(time.Date(2025, 1, 1, 0, 0, 0, 0, time.UTC)) || !p.To.Equal(time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)) {
		t.Fatalf("got %v to %v", p.From, p.To)
	}
}

func TestPeriodFromFallsBackToDays(t *testing.T) {
	for _, query := range []string{"?days=7", "?days=7&from=10", "?days=7&from=20&to=10", "?days=7&from=x&to=y"} {
		p := periodFrom(httptest.NewRequest("GET", "/v1/me/stats/summary"+query, nil))
		if got := p.To.Sub(p.From); got != 7*24*time.Hour {
			t.Fatalf("%s: got a span of %v", query, got)
		}
	}
}

package api

import (
	"bytes"
	"net/http"
	"net/http/httptest"
	"reflect"
	"testing"
	"time"

	"spotifier/internal/domain"
	"spotifier/internal/session"
)

func queueOf(state domain.PlayState, ids ...string) domain.Session {
	items := make([]domain.Track, len(ids))
	for i, id := range ids {
		items[i] = domain.Track{ID: id, Playable: true}
	}
	return domain.Session{State: state, Queue: domain.Queue{Items: items}}
}

func TestAMeteredNetworkFetchesOnlyWhatIsAboutToSound(t *testing.T) {
	s, _ := newWholeServer(t, &wholeUpstream{data: audioFile(streamWindow)}, &switchable{})
	playing := queueOf(domain.StatePlaying, "a", "b", "c", "d", "e", "f")
	paused := queueOf(domain.StatePaused, "a", "b", "c", "d", "e", "f")

	if got, want := s.QueueToFetch(playing), []string{"a", "b", "c", "d"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("unmetered: %v, want %v", got, want)
	}
	if got, want := s.QueueToFetch(paused), []string{"a", "b", "c", "d"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("unmetered, paused: %v, want %v", got, want)
	}

	s.SetMetered(true)
	if got, want := s.QueueToFetch(playing), []string{"a", "b"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("metered: %v, want %v", got, want)
	}
	if got := s.QueueToFetch(paused); len(got) != 0 {
		t.Fatalf("metered and paused: %v, want nothing", got)
	}
	last := queueOf(domain.StatePlaying, "a")
	if got, want := s.QueueToFetch(last), []string{"a"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("metered, one left: %v, want %v", got, want)
	}
}

// A guess is dropped on a metered network; the next track's fill, which a
// gapless or crossfaded change is waiting on, is not.
func TestAMeteredNetworkDropsGuessesButKeepsTheNextTrack(t *testing.T) {
	u := &wholeUpstream{data: audioFile(2 * streamWindow)}
	s, cache := newWholeServer(t, u, &switchable{})
	s.SetMetered(true)

	s.prefetchTrack("guessed004", true, false, "hover")
	time.Sleep(100 * time.Millisecond)
	if _, ok := cache.Get("guessed004"); ok || s.fillActive("guessed004") {
		t.Fatal("a guess was fetched on a metered network")
	}

	s.Prefetch("preload004", true)
	waitComplete(t, cache, "preload004")
}

func TestTheSettingsSayWhetherTheNetworkIsMetered(t *testing.T) {
	s, _ := newWholeServer(t, &wholeUpstream{data: audioFile(streamWindow)}, &switchable{})
	s.deps.Session = session.NewHub(nil, session.DefaultSettings(), nil)
	post := func(body string) {
		req := httptest.NewRequest(http.MethodPost, "/v1/session/settings", bytes.NewBufferString(body))
		rec := httptest.NewRecorder()
		s.handleSessionSettings(rec, req)
		if rec.Code != http.StatusOK {
			t.Fatalf("settings answered %d", rec.Code)
		}
	}
	post(`{"metered":true}`)
	if !s.prefetch.metered.Load() {
		t.Fatal("not metered after being told so")
	}
	post(`{"gapless":true}`)
	if !s.prefetch.metered.Load() {
		t.Fatal("a client that does not say changed it")
	}
	post(`{"metered":false}`)
	if s.prefetch.metered.Load() {
		t.Fatal("still metered after being told otherwise")
	}
}

// Everything read from upstream is counted, which is the figure to set against
// the phone's own count of its data use.
func TestUpstreamBytesAreCounted(t *testing.T) {
	u := &wholeUpstream{data: audioFile(3*streamWindow + 11)}
	s, cache := newWholeServer(t, u, &switchable{})
	s.Prefetch("counted004", true)
	waitComplete(t, cache, "counted004")
	if got, want := s.upstreamBytes.Load(), u.sent.Load(); got != want {
		t.Fatalf("counted %d bytes, upstream sent %d", got, want)
	}
}

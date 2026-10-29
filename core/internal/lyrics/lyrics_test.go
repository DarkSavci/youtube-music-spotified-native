package lyrics_test

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"reflect"
	"testing"

	"spotifier/internal/domain"
	"spotifier/internal/lyrics"
)

type fake struct {
	name string
	out  domain.Lyrics
	err  error
	hits int
}

func (f *fake) Name() string { return f.name }
func (f *fake) Lyrics(context.Context, domain.Track) (domain.Lyrics, error) {
	f.hits++
	return f.out, f.err
}

var track = domain.Track{
	ID:         "abc",
	Title:      "Creep",
	Artists:    []domain.ArtistRef{{Name: "Radiohead"}},
	DurationMs: 239_000,
}

/*
A timed result wins, whichever source has it.

Taking the first non-empty answer would always stop at the primary source and
discard the timings, which are the only reason to ask.
*/
func TestTimedResultWinsWhenTimingsAreWanted(t *testing.T) {
	timed := &fake{name: "LRCLIB", out: domain.Lyrics{
		Source: "LRCLIB",
		Lines:  []domain.LyricLine{{AtMs: 1000, Text: "When you were here before"}},
	}}
	plain := &fake{name: "YouTube Music", out: domain.Lyrics{Source: "Musixmatch", Plain: "words"}}

	got, err := (&lyrics.Service{Primary: plain, Timed: timed}).
		Lyrics(context.Background(), track, true)
	if err != nil {
		t.Fatal(err)
	}
	if !got.Synced {
		t.Fatal("returned an unsynced result although a synced one was available")
	}
	if plain.hits == 0 {
		t.Fatal("never asked the primary source, which usually has the timings")
	}
}

/*
YouTube's own timings mean the third party is not contacted at all.

It has timed lyrics for most of the catalogue now that they are requested as a
mobile client, so reaching outside YouTube when they are already in hand sends
the track title somewhere it did not need to go.
*/
func TestThirdPartyIsSkippedWhenThePrimaryHasTimings(t *testing.T) {
	primary := &fake{name: "YouTube Music", out: domain.Lyrics{
		Source: "YouTube Music",
		Lines:  []domain.LyricLine{{AtMs: 1000, Text: "one"}},
	}}
	timed := &fake{name: "LRCLIB", out: domain.Lyrics{Source: "LRCLIB", Plain: "x"}}

	got, err := (&lyrics.Service{Primary: primary, Timed: timed}).
		Lyrics(context.Background(), track, true)
	if err != nil {
		t.Fatal(err)
	}
	if !got.Synced || got.Source != "YouTube Music" {
		t.Fatalf("got %+v, want YouTube's timed lyrics", got)
	}
	if timed.hits != 0 {
		t.Fatal("contacted the third party although YouTube already had timings")
	}
}

// And when YouTube has only plain words, the third party is still tried for
// timings — otherwise enabling the setting would change nothing.
func TestThirdPartyIsTriedWhenThePrimaryIsUntimed(t *testing.T) {
	primary := &fake{name: "YouTube Music", out: domain.Lyrics{Source: "Musixmatch", Plain: "words"}}
	timed := &fake{name: "LRCLIB", out: domain.Lyrics{
		Source: "LRCLIB",
		Lines:  []domain.LyricLine{{AtMs: 500, Text: "timed"}},
	}}

	got, err := (&lyrics.Service{Primary: primary, Timed: timed}).
		Lyrics(context.Background(), track, true)
	if err != nil {
		t.Fatal(err)
	}
	if !got.Synced {
		t.Fatalf("returned untimed words although timings were available: %+v", got)
	}
}

// With timings turned off, YouTube's plain words are the answer and nothing
// leaves for a third party.
func TestPrimaryPlainIsKeptWhenTimingsAreNotWanted(t *testing.T) {
	primary := &fake{name: "YouTube Music", out: domain.Lyrics{Source: "Musixmatch", Plain: "words"}}
	timed := &fake{name: "LRCLIB", out: domain.Lyrics{Source: "LRCLIB", Plain: "other"}}

	got, err := (&lyrics.Service{Primary: primary, Timed: timed}).
		Lyrics(context.Background(), track, false)
	if err != nil {
		t.Fatal(err)
	}
	if got.Source != "Musixmatch" || timed.hits != 0 {
		t.Fatalf("got %+v, timed hits %d", got, timed.hits)
	}
}

// And it is never contacted at all when timings are not wanted, so nothing
// about what is playing reaches a third party.
func TestTimedSourceIsNotContactedWhenNotWanted(t *testing.T) {
	timed := &fake{name: "LRCLIB", out: domain.Lyrics{Plain: "x"}}
	plain := &fake{name: "YouTube Music", out: domain.Lyrics{Source: "Musixmatch", Plain: "words"}}

	got, err := (&lyrics.Service{Primary: plain, Timed: timed}).
		Lyrics(context.Background(), track, false)
	if err != nil {
		t.Fatal(err)
	}
	if timed.hits != 0 {
		t.Fatal("contacted the third-party source although timings were not wanted")
	}
	if got.Source != "Musixmatch" {
		t.Fatalf("source = %q, want the primary's attribution", got.Source)
	}
}

// A source with nothing for this track falls through to the next.
func TestFallsThroughAnEmptySource(t *testing.T) {
	timed := &fake{name: "LRCLIB", err: lyrics.ErrNotFound}
	plain := &fake{name: "YouTube Music", out: domain.Lyrics{Source: "Musixmatch", Plain: "words"}}

	got, err := (&lyrics.Service{Primary: plain, Timed: timed}).
		Lyrics(context.Background(), track, true)
	if err != nil {
		t.Fatal(err)
	}
	if got.Plain != "words" {
		t.Fatalf("plain = %q", got.Plain)
	}
}

// No lyrics anywhere is an ordinary outcome, not a failure.
func TestNoLyricsAnywhere(t *testing.T) {
	s := &lyrics.Service{
		Primary: &fake{name: "a", err: lyrics.ErrNotFound},
		Timed:   &fake{name: "b", err: lyrics.ErrNotFound},
	}
	if _, err := s.Lyrics(context.Background(), track, true); !errors.Is(err, lyrics.ErrNotFound) {
		t.Fatalf("err = %v, want ErrNotFound", err)
	}
}

// A timed result must still carry plain text, so a consumer that only prints
// text works without knowing about timings.
func TestTimedResultAlsoCarriesPlainText(t *testing.T) {
	timed := &fake{name: "LRCLIB", out: domain.Lyrics{
		Source: "LRCLIB",
		Lines: []domain.LyricLine{
			{AtMs: 0, Text: "one"},
			{AtMs: 1000, Text: "two"},
		},
	}}
	got, err := (&lyrics.Service{Timed: timed}).Lyrics(context.Background(), track, true)
	if err != nil {
		t.Fatal(err)
	}
	if got.Plain != "one\ntwo" {
		t.Fatalf("plain = %q, want the lines joined", got.Plain)
	}
}

/* ---------- LRC parsing ---------- */

func TestParseLRC(t *testing.T) {
	const lrc = "[ar:Radiohead]\n" +
		"[ti:Creep]\n" +
		"[00:14.20]When you were here before\n" +
		"[00:18.5]Couldn't look you in the eye\n" +
		"[01:02.123]You're just like an angel\n" +
		"\n" +
		"[02:00.00]\n" +
		"[00:20.00]Out of order line\n"

	lines := lyrics.ParseLRC(lrc)
	if len(lines) != 5 {
		t.Fatalf("parsed %d lines, want 5: %+v", len(lines), lines)
	}
	// Metadata tags must not become lyrics.
	for _, l := range lines {
		if l.Text == "Radiohead" || l.Text == "Creep" {
			t.Fatalf("metadata tag parsed as a line: %+v", l)
		}
	}
	if lines[0].AtMs != 14_200 {
		t.Fatalf("first line at %dms, want 14200", lines[0].AtMs)
	}
	// One-digit fractions are tenths.
	if lines[1].AtMs != 18_500 {
		t.Fatalf("second line at %dms, want 18500", lines[1].AtMs)
	}
	// Out-of-order input must be sorted, or lines show early.
	for i := 1; i < len(lines); i++ {
		if lines[i].AtMs < lines[i-1].AtMs {
			t.Fatalf("lines not in time order: %+v", lines)
		}
	}
	// An empty timed line is a real pause and is kept.
	var blanks int
	for _, l := range lines {
		if l.Text == "" {
			blanks++
		}
	}
	if blanks != 1 {
		t.Fatalf("kept %d blank lines, want 1", blanks)
	}
}

func TestParseLRCReadsWordTimings(t *testing.T) {
	const lrc = "[00:12.00]<00:12.00>Hello <00:12.50>there <00:13.10>world<00:14.00>\n" +
		"[00:20.00][00:40.00]<00:20.00>Sing <00:20.40>again\n" +
		"[00:30.00]<00:30.00>Broken <bad>tag\n"

	lines := lyrics.ParseLRC(lrc)
	if len(lines) != 4 {
		t.Fatalf("parsed %d lines, want 4: %+v", len(lines), lines)
	}
	first := lines[0]
	if first.Text != "Hello there world" {
		t.Fatalf("text = %q, want the words without their tags", first.Text)
	}
	want := []domain.LyricWord{{AtMs: 12_000, Text: "Hello "}, {AtMs: 12_500, Text: "there "}, {AtMs: 13_100, Text: "world"}}
	if !reflect.DeepEqual(first.Words, want) {
		t.Fatalf("words = %+v, want %+v", first.Words, want)
	}
	// A repeated line is sung the same, later.
	if got := lines[3].Words; lines[3].AtMs != 40_000 || len(got) != 2 || got[0].AtMs != 40_000 || got[1].AtMs != 40_400 {
		t.Fatalf("repeat = %+v", lines[3])
	}
	// One bad tag leaves the line plain rather than wrongly timed.
	if broken := lines[2]; broken.Words != nil || broken.Text != "Broken <bad>tag" {
		t.Fatalf("half-timed line kept words: %+v", broken)
	}
	// Plain lyrics carry no words at all.
	if plain := lyrics.ParseLRC("[00:01.00]just a line"); plain[0].Words != nil {
		t.Fatalf("plain line has words: %+v", plain[0])
	}
}

func TestParseLyricsfileReadsWords(t *testing.T) {
	const file = "version: '1.0'\n" +
		"metadata:\n  title: Song\n  artist: Someone\n" +
		"lines:\n" +
		"- text: Later line\n  start_ms: 9000\n" +
		"- text: Hello world\n  start_ms: 1000\n  end_ms: 3000\n  words:\n" +
		"  - text: 'Hello '\n    start_ms: 1000\n  - text: world\n    start_ms: 1600\n"

	lines := lyrics.ParseLyricsfile(file)
	if len(lines) != 2 || lines[0].Text != "Hello world" || lines[1].AtMs != 9000 {
		t.Fatalf("lines = %+v", lines)
	}
	want := []domain.LyricWord{{AtMs: 1000, Text: "Hello "}, {AtMs: 1600, Text: "world"}}
	if !reflect.DeepEqual(lines[0].Words, want) {
		t.Fatalf("words = %+v, want %+v", lines[0].Words, want)
	}
	if lyrics.ParseLyricsfile("lines: [unclosed") != nil {
		t.Fatal("a broken file gave lines")
	}
	// Aliases are refused: nested, they turn a small file into a huge one.
	const bomb = "a: &a [x, x, x]\nb: &b [*a, *a, *a]\nlines:\n- text: x\n  start_ms: 1\n  words: *b\n"
	if lyrics.ParseLyricsfile(bomb) != nil {
		t.Fatal("a file with aliases gave lines")
	}
}

func TestParseLRCIgnoresJunk(t *testing.T) {
	for _, in := range []string{"", "   ", "no timestamps here", "[not a stamp]words", "[99:99.99]bad"} {
		if got := lyrics.ParseLRC(in); len(got) != 0 {
			t.Fatalf("ParseLRC(%q) = %+v, want nothing", in, got)
		}
	}
}

/* ---------- LRCLIB adapter ---------- */

func TestLRCLibReturnsSyncedLyrics(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if got := r.URL.Query().Get("track_name"); got != "Creep" {
			t.Errorf("track_name = %q", got)
		}
		if got := r.URL.Query().Get("artist_name"); got != "Radiohead" {
			t.Errorf("artist_name = %q", got)
		}
		if got := r.URL.Query().Get("duration"); got != "239" {
			t.Errorf("duration = %q, want seconds", got)
		}
		_, _ = w.Write([]byte(`{"plainLyrics":"a\nb","syncedLyrics":"[00:01.00]a\n[00:02.00]b"}`))
	}))
	defer srv.Close()

	l := lyrics.NewLRCLib()
	l.BaseURL = srv.URL
	got, err := l.Lyrics(context.Background(), track)
	if err != nil {
		t.Fatal(err)
	}
	if len(got.Lines) != 2 {
		t.Fatalf("lines = %+v", got.Lines)
	}
	if got.Source != "LRCLIB" {
		t.Fatalf("source = %q; attribution is required", got.Source)
	}
}

func TestLRCLibTreatsInstrumentalAsNoLyrics(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write([]byte(`{"instrumental":true,"plainLyrics":""}`))
	}))
	defer srv.Close()

	l := lyrics.NewLRCLib()
	l.BaseURL = srv.URL
	if _, err := l.Lyrics(context.Background(), track); !errors.Is(err, lyrics.ErrNotFound) {
		t.Fatalf("err = %v, want ErrNotFound", err)
	}
}

// Without a title and artist the lookup cannot be specific enough to trust,
// and asking anyway would send a useless request.
func TestLRCLibSkipsWhenTheTrackIsUnidentified(t *testing.T) {
	var called bool
	srv := httptest.NewServer(http.HandlerFunc(func(http.ResponseWriter, *http.Request) {
		called = true
	}))
	defer srv.Close()

	l := lyrics.NewLRCLib()
	l.BaseURL = srv.URL
	_, err := l.Lyrics(context.Background(), domain.Track{ID: "x", Title: "Only a title"})
	if !errors.Is(err, lyrics.ErrNotFound) {
		t.Fatalf("err = %v, want ErrNotFound", err)
	}
	if called {
		t.Fatal("sent a request for a track it could not identify")
	}
}

func TestLRCLibReadsWordSync(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte(`{"plainLyrics":"Hi you","syncedLyrics":"[00:01.00]Hi you","hasWordSync":true,` +
			`"lyricsfile":"lines:\n- text: Hi you\n  start_ms: 1000\n  words:\n  - text: 'Hi '\n    start_ms: 1000\n  - text: you\n    start_ms: 1400\n"}`))
	}))
	defer srv.Close()

	l := lyrics.NewLRCLib()
	l.BaseURL = srv.URL
	got, err := l.Lyrics(context.Background(), track)
	if err != nil {
		t.Fatal(err)
	}
	if len(got.Lines) != 1 || len(got.Lines[0].Words) != 2 || got.Lines[0].Words[1].AtMs != 1400 {
		t.Fatalf("lines = %+v", got.Lines)
	}
}

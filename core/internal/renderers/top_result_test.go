package renderers

import (
	"testing"

	"spotifier/internal/domain"
)

// The top result is what the card itself points at — a song plays, an album
// opens — not whichever link a search of the card turns up first. Its
// subtitle links the artist, and a map walk found that about half the time,
// so a song or album top result opened the artist.
func TestTopResultIsWhatTheCardPointsAt(t *testing.T) {
	cases := []struct {
		fixture string
		kind    domain.ShelfItemKind
		id      string
	}{
		{"search_empty", domain.KindTrack, "pgRIJ5efW4g"},
		{"search_unicode", domain.KindAlbum, "MPREb_CEsAJuezLwa"},
		{"search_all", domain.KindArtist, "UCRr1xG_2WIDs18a6cIiCxeA"},
	}
	for _, c := range cases {
		doc := loadFixture(t, c.fixture)
		for attempt := range 60 {
			res := ParseSearch(doc, "q", ParseContext{})
			top := res.TopResult
			if top == nil {
				t.Fatalf("%s: no top result", c.fixture)
			}
			id := ""
			switch top.Kind {
			case domain.KindTrack:
				id = top.Track.ID
			case domain.KindAlbum:
				id = top.Album.ID
			case domain.KindArtist:
				id = top.Artist.ID
			}
			if top.Kind != c.kind || id != c.id {
				t.Fatalf("%s attempt %d: got %s %s, want %s %s", c.fixture, attempt, top.Kind, id, c.kind, c.id)
			}
		}
	}
}

// The card holds songs with covers of their own, and its picture is the one
// it carries itself: a search of the whole card found a song's cover first
// about half the time, and showed an artist with a single's sleeve.
func TestTopResultWearsItsOwnPicture(t *testing.T) {
	doc := loadFixture(t, "search_all")
	card := Find(doc, NodeCardShelf)
	want := card.Child("thumbnail").FindArtwork()
	if len(want) == 0 {
		t.Fatal("the fixture's card has no picture of its own")
	}
	for attempt := range 60 {
		top := ParseSearch(doc, "q", ParseContext{}).TopResult
		if top == nil || top.Artist == nil {
			t.Fatal("no artist as the top result")
		}
		got := top.Artist.Artwork
		if len(got) != len(want) || got[0].URL != want[0].URL {
			t.Fatalf("attempt %d: got %v, want %v", attempt, got, want)
		}
	}
}

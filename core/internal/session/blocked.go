package session

import (
	"strings"

	"spotifier/internal/domain"
)

/*
Blocked songs, artists and albums: what the listener never wants played.

A blocked track stays in the queue, where it can be seen and unblocked, and is
stepped over whenever the queue moves: at the end of a song, on next and
previous, and when naming the track to have ready, so a gapless or crossfaded
transition never lets a second of it through. A room's queue is the room's, and
plays as the room has it.

The one way to hear what is blocked is to ask for it and nothing else. A queue
of a blocked song alone, or of a blocked album from its own page, plays as it
stands, since stepping over all of it would leave nothing.
*/

// Blocked names what is never to be played, by id.
type Blocked struct {
	Tracks  []string `json:"tracks"`
	Artists []string `json:"artists"`
	Albums  []string `json:"albums"`
}

// SetBlocked replaces what is blocked. A track that is playing and has just
// been blocked gives way to the next.
func (c *Core) SetBlocked(b Blocked) []LogEntry {
	cur := c.state.Queue.Current()
	was := cur != nil && c.blocked(cur)

	c.blockedTracks = make(map[string]bool, len(b.Tracks))
	for _, id := range b.Tracks {
		c.blockedTracks[id] = true
	}
	c.blockedArtists = make(map[string]bool, len(b.Artists))
	for _, id := range b.Artists {
		c.blockedArtists[artistKey(id)] = true
	}
	c.blockedAlbums = make(map[string]bool, len(b.Albums))
	for _, id := range b.Albums {
		c.blockedAlbums[id] = true
	}

	// Only one blocked by this change: a blocked song played by itself was
	// asked for, and every later change of settings must leave it playing.
	if cur != nil && !was && c.blocked(cur) && !c.following && c.playIntent() {
		if !c.anyAllowed() {
			// The whole queue went with it: an album blocked while it plays.
			c.state.PositionMs = c.positionNow()
			c.state.PositionAt = c.clk.Now()
			c.state.State = domain.StatePaused
			c.bump()
			return nil
		}
		_, logs := c.skip(+1, true)
		return logs
	}
	c.bump()
	return nil
}

// Blocked is whether the listener has blocked this track, its album or one of
// its artists.
func (c *Core) Blocked(t *domain.Track) bool { return c.blocked(t) }

func (c *Core) blocked(t *domain.Track) bool {
	if len(c.blockedTracks) == 0 && len(c.blockedArtists) == 0 && len(c.blockedAlbums) == 0 {
		return false
	}
	if c.blockedTracks[t.ID] {
		return true
	}
	if t.Album != nil && t.Album.ID != "" && c.blockedAlbums[t.Album.ID] {
		return true
	}
	for _, a := range t.Artists {
		if a.ID != "" && c.blockedArtists[artistKey(a.ID)] {
			return true
		}
	}
	return false
}

// artistKey is the channel an artist id names. One from the library arrives
// wrapped ("MPLA" before the channel's id), and is the same artist.
func artistKey(id string) string {
	if channel, ok := strings.CutPrefix(id, "MPLA"); ok && strings.HasPrefix(channel, "UC") {
		return channel
	}
	return id
}

// firstAllowed is the first track at or after idx that is not blocked, or idx
// itself when every one from there on is.
func (c *Core) firstAllowed(items []domain.Track, idx int) int {
	for i := idx; i < len(items); i++ {
		if !c.blocked(&items[i]) {
			return i
		}
	}
	return idx
}

// anyAllowed is whether the queue holds a track that is not blocked. One
// that holds none was asked for as it is, and plays as it is.
func (c *Core) anyAllowed() bool {
	for i := range c.state.Queue.Items {
		if !c.blocked(&c.state.Queue.Items[i]) {
			return true
		}
	}
	return false
}

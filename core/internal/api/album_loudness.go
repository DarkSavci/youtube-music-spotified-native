package api

import (
	"context"
	"math"
	"sync"

	"spotifier/internal/domain"
)

// albumReach is how many tracks either side of one are looked at for the
// rest of its album: more than any album has, few enough that a long
// queue of one artist's albums costs nothing much.
const albumReach = 40

// albumLookups is how many loudness lookups run at once for an album.
const albumLookups = 6

/*
albumLoudness is how loud the album a track belongs to is, taken as the
tracks around it in the queue that share its album.

Evening out each track on its own brings a quiet interlude up to the song
after it — or, as this core only ever turns down, brings a loud song down to
the interlude. One gain for the whole album keeps the difference the album
was made with. The album's loudness is the tracks' loudness averaged as
energy, each weighted by its length, which is what measuring the album
straight through would give.

ok is false when the track is not in the queue, has no album, or is its
album's only track there: its own loudness is then the album's. The lookups
run together and land in the loudness service's memory, so the album's next
track asks for nothing new.
*/
func albumLoudness(ctx context.Context, queue []domain.Track, videoID string, lookup func(context.Context, string) (float64, error)) (lkfs float64, ok bool) {
	at := -1
	for i := range queue {
		if queue[i].ID == videoID {
			at = i
			break
		}
	}
	if at < 0 || queue[at].Album == nil || queue[at].Album.ID == "" {
		return 0, false
	}
	album := queue[at].Album.ID
	same := func(i int) bool {
		return i >= 0 && i < len(queue) && queue[i].Album != nil && queue[i].Album.ID == album
	}
	first, last := at, at
	for first > at-albumReach && same(first-1) {
		first--
	}
	for last < at+albumReach && same(last+1) {
		last++
	}
	run := queue[first : last+1]
	if len(run) < 2 {
		return 0, false
	}

	var (
		mu     sync.Mutex
		energy float64
		weight float64
		wg     sync.WaitGroup
		slots  = make(chan struct{}, albumLookups)
	)
	for _, t := range run {
		wg.Add(1)
		go func(t domain.Track) {
			defer wg.Done()
			slots <- struct{}{}
			defer func() { <-slots }()
			l, err := lookup(ctx, t.ID)
			if err != nil {
				return
			}
			w := float64(t.DurationMs)
			if w <= 0 {
				w = 1
			}
			mu.Lock()
			energy += w * math.Pow(10, l/10)
			weight += w
			mu.Unlock()
		}(t)
	}
	wg.Wait()
	if weight == 0 {
		return 0, false
	}
	return 10 * math.Log10(energy/weight), true
}

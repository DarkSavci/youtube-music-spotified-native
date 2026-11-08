package api

import (
	"context"
	"errors"
	"math"
	"testing"

	"spotifier/internal/domain"
)

func TestAnAlbumIsAsLoudAsItsTracksTogether(t *testing.T) {
	a := &domain.AlbumRef{ID: "a", Name: "A"}
	b := &domain.AlbumRef{ID: "b", Name: "B"}
	queue := []domain.Track{
		{ID: "x", Album: b, DurationMs: 1000},
		{ID: "loud", Album: a, DurationMs: 1000},
		{ID: "quiet", Album: a, DurationMs: 1000},
		{ID: "lost", Album: a, DurationMs: 1000},
		{ID: "y", Album: b, DurationMs: 1000},
		{ID: "single", DurationMs: 1000},
	}
	levels := map[string]float64{"loud": -8, "quiet": -20, "x": -30, "y": -30, "single": -9}
	lookup := func(_ context.Context, id string) (float64, error) {
		if l, ok := levels[id]; ok {
			return l, nil
		}
		return 0, errors.New("unknown")
	}

	got, ok := albumLoudness(context.Background(), queue, "quiet", lookup)
	// The two that answered, as energy: the loud one dominates.
	want := 10 * math.Log10((math.Pow(10, -0.8)+math.Pow(10, -2))/2)
	if !ok || math.Abs(got-want) > 1e-9 {
		t.Fatalf("album loudness %v %v, want %v", got, ok, want)
	}
	// Tracks of another album, and one with none, are their own.
	if _, ok := albumLoudness(context.Background(), queue, "x", lookup); ok {
		t.Error("a track alone in the queue from its album was given an album figure")
	}
	if _, ok := albumLoudness(context.Background(), queue, "single", lookup); ok {
		t.Error("a track without an album was given an album figure")
	}
	if _, ok := albumLoudness(context.Background(), queue, "absent", lookup); ok {
		t.Error("a track not in the queue was given an album figure")
	}
}

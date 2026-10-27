package resolver

import "sync/atomic"

/*
How good a stream is asked for.

The listener chooses, per network on a phone: the best there is on Wi-Fi,
something lighter on mobile data. It is a ceiling on bitrate that the
adapters apply when they pick a format; a resolution already made keeps its
format until it expires.
*/

// Ceilings, in bits per second. High is no ceiling at all.
const (
	QualityHigh     = 0
	QualityStandard = 170_000
	QualityLow      = 80_000
)

var maxBitrate atomic.Int64

// SetQuality sets the ceiling by name: "high", "standard" or "low".
// Anything else is high.
func SetQuality(name string) {
	switch name {
	case "standard":
		maxBitrate.Store(QualityStandard)
	case "low":
		maxBitrate.Store(QualityLow)
	default:
		maxBitrate.Store(QualityHigh)
	}
}

// ceiling is the highest bitrate to pick, or 0 for no limit.
func ceiling() int64 { return maxBitrate.Load() }

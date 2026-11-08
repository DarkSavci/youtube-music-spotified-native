package resolver

import "testing"

func TestAFormatIsChosenForHowItSoundsBeforeItsBitrate(t *testing.T) {
	formats := []adaptiveFormat{
		{Itag: 140, MimeType: `audio/mp4; codecs="mp4a.40.2"`, Bitrate: 130_000, URL: "aac"},
		{Itag: 251, MimeType: `audio/webm; codecs="opus"`, Bitrate: 160_000, URL: "drc", IsDrc: true},
		{Itag: 251, MimeType: `audio/webm; codecs="opus"`, Bitrate: 150_000, URL: "opus"},
	}
	// The DRC copy states more, and still loses to the full range.
	if got := bestAdaptiveAudio(formats).URL; got != "opus" {
		t.Fatalf("standard tier: got %s", got)
	}
	premium := append(formats,
		adaptiveFormat{Itag: 141, MimeType: `audio/mp4; codecs="mp4a.40.2"`, Bitrate: 290_000, URL: "aac-premium"},
		adaptiveFormat{Itag: 774, MimeType: `audio/webm; codecs="opus"`, Bitrate: 260_000, URL: "opus-premium"},
	)
	// Opus over AAC at the same tier, though the AAC states more.
	if got := bestAdaptiveAudio(premium).URL; got != "opus-premium" {
		t.Fatalf("premium tier: got %s", got)
	}
}

func TestYtdlpFormatsAreRankedTheSameWay(t *testing.T) {
	formats := []ytdlpFormat{
		{URL: "aac-premium", FormatID: "141", ACodec: "mp4a.40.2", VCodec: "none", ABR: 290, Protocol: "https"},
		{URL: "opus-drc", FormatID: "251-drc", ACodec: "opus", VCodec: "none", ABR: 160, Protocol: "https"},
		{URL: "opus-premium", FormatID: "774", ACodec: "opus", VCodec: "none", ABR: 256, Protocol: "https"},
	}
	if got := bestYtdlpAudio(formats).URL; got != "opus-premium" {
		t.Fatalf("got %s", got)
	}
	if got := bestYtdlpAudio(formats[1:2]).URL; got != "opus-drc" {
		t.Fatalf("a DRC copy alone is still played: got %s", got)
	}
}

// Under a ceiling, a far lighter Opus does not win for being Opus.
func TestUnderACeilingAFarLighterOpusLosesToAAC(t *testing.T) {
	SetQuality("standard")
	defer SetQuality("high")
	formats := []adaptiveFormat{
		{Itag: 251, MimeType: `audio/webm; codecs="opus"`, Bitrate: 175_000, URL: "opus-over"},
		{Itag: 140, MimeType: `audio/mp4; codecs="mp4a.40.2"`, Bitrate: 130_000, URL: "aac"},
		{Itag: 250, MimeType: `audio/webm; codecs="opus"`, Bitrate: 70_000, URL: "opus-light"},
	}
	if got := bestAdaptiveAudio(formats).URL; got != "aac" {
		t.Fatalf("got %s", got)
	}
	// Close enough, Opus still wins.
	formats[0].Bitrate = 150_000
	if got := bestAdaptiveAudio(formats).URL; got != "opus-over" {
		t.Fatalf("got %s", got)
	}
}

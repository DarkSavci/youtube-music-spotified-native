package resolver

import "testing"

func TestVideoFormatsDoNotReplaceAudioOrChooseManifests(t *testing.T) {
	formats := []ytdlpFormat{
		{URL: "audio", Ext: "webm", ACodec: "opus", VCodec: "none", ABR: 160, Protocol: "https"},
		{URL: "4k", Ext: "mp4", VCodec: "avc1", Height: 2160, Protocol: "https"},
		{URL: "hls", Ext: "mp4", VCodec: "avc1", Height: 1080, Protocol: "m3u8_native"},
		{URL: "picture", Ext: "mp4", VCodec: "avc1", Height: 1080, Protocol: "https"},
		{URL: "lower", Ext: "mp4", VCodec: "avc1", Height: 720, Protocol: "https"},
	}
	if bestYtdlpVideo(formats, VideoPick{}).URL != "picture" {
		t.Fatal("wrong video format")
	}
	if bestYtdlpAudio(formats).URL != "audio" {
		t.Fatal("audio format selection changed")
	}
	if bestYtdlpVideo(formats[:1], VideoPick{}) != nil {
		t.Fatal("audio-only format became a video")
	}
}

func TestVideoPickKeepsToACodecAndHeight(t *testing.T) {
	formats := []ytdlpFormat{
		{URL: "vp9-1080", Ext: "webm", VCodec: "vp09.00.40.08", Height: 1080, TBR: 5000, Protocol: "https"},
		{URL: "av1-1080", Ext: "mp4", VCodec: "av01.0.08M.08", Height: 1080, TBR: 2000, Protocol: "https"},
		{URL: "avc-1080", Ext: "mp4", VCodec: "avc1.640028", Height: 1080, TBR: 4000, Protocol: "https"},
		{URL: "avc-720", Ext: "mp4", VCodec: "avc1.64001f", Height: 720, TBR: 2000, Protocol: "https"},
		{URL: "avc-hls", Ext: "mp4", VCodec: "avc1.64001f", Height: 720, TBR: 9000, Protocol: "m3u8_native"},
	}
	for _, tc := range []struct {
		pick VideoPick
		want string
	}{
		{VideoPick{}, "vp9-1080"},
		{VideoPick{Codec: "h264"}, "avc-1080"},
		{VideoPick{Codec: "h264", MaxHeight: 720}, "avc-720"},
		{VideoPick{Codec: "h264", MaxHeight: 5000}, "avc-1080"},
	} {
		got := bestYtdlpVideo(formats, tc.pick)
		if got == nil || got.URL != tc.want {
			t.Fatalf("%+v chose %+v, want %s", tc.pick, got, tc.want)
		}
	}
	if bestYtdlpVideo(formats[:2], VideoPick{Codec: "h264"}) != nil {
		t.Fatal("a codec the client cannot decode was offered as H.264")
	}
	if got := (VideoPick{Codec: "h264", MaxHeight: 720}).selector(); got != "bestvideo[protocol^=http][height<=720][vcodec^=avc1]/best[protocol^=http][height<=720][vcodec^=avc1]" {
		t.Fatalf("selector %q", got)
	}
	if got := (VideoPick{}).selector(); got != "bestvideo[protocol^=http][height<=1080]/best[protocol^=http][height<=1080]" {
		t.Fatalf("the default selector changed: %q", got)
	}
}

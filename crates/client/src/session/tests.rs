//! Tests of the session's wire forms.

use super::*;

/// As the core sent it for a two-track queue, trimmed.
const PROJECTION: &str = r#"{"followingRoom":false,"state":{"version":2,"epoch":1,
    "queue":{"items":[{"id":"abc","title":"T","artists":[{"name":"A"}],"durationMs":1000,
    "artwork":[],"explicit":false,"isVideo":false,"playable":true}],"index":0,"origin":"Test"},
    "state":"playing","repeat":"off","shuffle":false,"volume":0.5,"positionMs":0,
    "positionAt":"2026-10-03T21:34:52.4041335+03:00","ownerDeviceId":"probe"},
    "target":{"Epoch":1,"VideoID":"abc","StartAtMs":0,"Playing":true,"PreloadVideoID":"def",
    "Volume":0.5,"Transition":{"Kind":"gapless","Ms":0},"UserChange":true},
    "devices":[],"offline":false}"#;

#[test]
fn a_projection_reads_both_of_the_cores_spellings() {
    let projection: Projection = serde_json::from_str(PROJECTION).expect("a projection");
    assert_eq!(projection.state.version, 2);
    assert_eq!(projection.state.state, PlayState::Playing);
    assert_eq!(
        projection.state.current().map(|track| track.id.as_str()),
        Some("abc")
    );
    assert_eq!(projection.target.video_id, "abc");
    assert_eq!(projection.target.preload_video_id, "def");
    assert_eq!(projection.target.transition.kind, "gapless");
    assert_eq!(projection.target.transition.crossfade_ms(), 0);
    assert!(projection.target.playing);
}

#[test]
fn commands_are_written_with_go_field_names() {
    assert_eq!(
        Command::Seek(61_000).to_wire(),
        json!({ "Kind": "seek", "PositionMs": 61_000 })
    );
    assert_eq!(
        Command::SetRepeat(Repeat::One).to_wire(),
        json!({ "Kind": "set_repeat", "Repeat": "one" })
    );
    let play = Command::Play {
        tracks: vec![Track {
            id: "abc".into(),
            duration_ms: 1000,
            ..Track::default()
        }],
        start_index: 0,
        origin: "Test".into(),
    }
    .to_wire();
    assert_eq!(play["Tracks"][0]["id"], "abc");
    let append = Command::Enqueue {
        tracks: vec![Track::default()],
        at: None,
    };
    assert_eq!(append.to_wire()["At"], -1);
    assert_eq!(play["Tracks"][0]["durationMs"], 1000);
}

#[test]
fn switching_to_the_video_names_the_song_it_replaces() {
    let switch = Command::SwitchVariant {
        expected: "song1234567".into(),
        track: Box::new(Track {
            id: "clip1234567".into(),
            is_video: true,
            playable: true,
            ..Track::default()
        }),
    }
    .to_wire();
    assert_eq!(switch["Kind"], "switch_variant");
    assert_eq!(switch["ExpectedID"], "song1234567");
    assert_eq!(switch["Tracks"].as_array().map(Vec::len), Some(1));
    assert_eq!(switch["Tracks"][0]["id"], "clip1234567");
    assert_eq!(switch["Tracks"][0]["isVideo"], true);
    assert_eq!(switch["Tracks"][0]["playable"], true);
}

#[test]
fn an_engine_event_is_written_with_go_field_names() {
    let event = EngineEvent {
        kind: EngineEventKind::Position,
        epoch: 3,
        position_ms: 1500,
        duration_ms: 0,
        reason: String::new(),
    };
    assert_eq!(
        serde_json::to_value(&event).expect("json"),
        json!({"Kind":"position","Epoch":3,"PositionMs":1500,"DurationMs":0,"Reason":""})
    );
}

#[test]
fn the_event_stream_yields_each_projection_and_skips_keep_alives() {
    let one_line = PROJECTION.replace('\n', "");
    let stream = format!(
        ": keep-alive\n\nevent: projection\ndata: {one_line}\n\n: keep-alive\n\n\
         event: projection\ndata: {{not json\n\nevent: projection\ndata: {one_line}\n\n"
    );
    let mut seen = 0;
    read_events(stream.as_bytes(), |projection| {
        assert_eq!(projection.target.video_id, "abc");
        seen += 1;
    });
    assert_eq!(seen, 2);
}

#[test]
fn a_queue_put_in_place_is_a_play_that_does_not_start() {
    let load = Command::Load {
        tracks: Vec::new(),
        start_index: 2,
        origin: "Phone".into(),
    };
    let wire = load.to_wire();
    assert_eq!(wire["Kind"], "play");
    assert_eq!(wire["Paused"], true);
    assert_eq!(wire["StartIndex"], 2);
    assert_eq!(wire["Origin"], "Phone");
}

#[test]
fn settings_are_written_as_the_core_reads_them() {
    let settings = Settings {
        crossfade_ms: 6000,
        gapless: true,
        resume_on_launch: false,
        report_to_youtube: true,
        cache_max_mb: 2048,
        autoplay: false,
    };
    assert_eq!(
        json!(settings),
        json!({
            "crossfadeMs": 6000,
            "gapless": true,
            "resumeOnLaunch": false,
            "reportToYouTube": true,
            "cacheMaxMB": 2048,
            "autoplay": false,
        })
    );
}

#[test]
fn only_a_cut_waits_for_the_core_at_the_end_of_a_track() {
    let kind = |kind: &str| Transition {
        kind: kind.into(),
        ms: 0,
    };
    assert!(kind("gapless").runs_on());
    assert!(kind("crossfade").runs_on());
    assert!(!kind("cut").runs_on());
}

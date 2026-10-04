//! Plays a track through the engine against a running core and prints what
//! the engine reports. A check of the whole audio path without the app.
//!
//!     cargo run -p spotified-audio --example play -- http://127.0.0.1:PORT VIDEO_ID [NEXT_ID FIRST_LENGTH_MS]
//!
//! Plays quietly for a few seconds, seeks, pauses, resumes, then stops.

use std::time::Duration;

use spotified_audio::engine::{Engine, Target};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [origin, video_id, rest @ ..] = args.as_slice() else {
        return Err(
            "usage: play <core origin> <video id> [next video id, first length in ms]".into(),
        );
    };
    let began = std::time::Instant::now();
    let engine = Engine::start(origin.clone(), true, move |event| {
        println!("[{:7.3}] {event:?}", began.elapsed().as_secs_f64());
    })?;

    let mut target = Target {
        epoch: 1,
        video_id: video_id.clone(),
        preload_video_id: rest.first().cloned().unwrap_or_default(),
        playing: true,
        volume: 0.25,
        gapless: true,
        ..Target::default()
    };
    let step = |label: &str, target: &Target, seconds: u64| {
        println!("-- {label}");
        engine.apply(target.clone());
        std::thread::sleep(Duration::from_secs(seconds));
    };
    step("play", &target, 8);
    target.start_at_ms = 60_000;
    step("seek to 1:00", &target, 4);
    target.playing = false;
    step("pause", &target, 2);
    target.playing = true;
    step("resume", &target, 3);
    // With a next track and the first one's length: run off the end of the
    // first into the second, then name the second as the core would.
    if let [next, duration_ms] = rest {
        // With a four-second crossfade, eight seconds from the end: the
        // end is reported four seconds in, as the fade begins.
        let duration_ms = duration_ms.parse::<u64>()?;
        target.duration_ms = duration_ms;
        target.crossfade_ms = 4000;
        target.start_at_ms = duration_ms.saturating_sub(8000);
        step(
            "seek to eight seconds before the end, crossfade on",
            &target,
            6,
        );
        target = Target {
            epoch: 2,
            video_id: next.clone(),
            preload_video_id: String::new(),
            start_at_ms: 0,
            ..target
        };
        step("the core names the next track", &target, 3);
    }
    step("stop", &Target::default(), 1);
    Ok(())
}

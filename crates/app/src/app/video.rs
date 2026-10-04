//! Keeps the music video's picture on screen: starts and stops the decoder
//! as the views show or stop showing a surface, hands it the song's
//! position, and puts the picture that is due into one texture.

use std::time::{Duration, Instant};

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions, vec2};
use spotified_client::Client;

use super::App;
use crate::actions::{Action, VideoAsk};
use crate::sidecar::CoreStatus;
use crate::state::Playback;
use crate::video::{Clock, LET_GO_AFTER, Player, Status};

/// How soon to look again when no picture is waiting: one picture's time
/// at the usual rates.
const LOOK_AGAIN: Duration = Duration::from_millis(33);
/// How far ahead of its time a picture may be shown. The window wakes a
/// few milliseconds either side of when it asked to, and a picture that is
/// nearly due is better shown now than waited for with another frame. Six
/// milliseconds early is not seen.
const EARLY: i64 = 6 * PER_MS;
/// The units the decoder counts time in, to a millisecond.
const PER_MS: i64 = 10_000;

#[derive(Default)]
pub(super) struct Screen {
    running: Option<Running>,
    texture: Option<TextureHandle>,
    /// When a surface was last on screen.
    watched: Option<Instant>,
    kept: Kept,
}

struct Running {
    /// The track and the count of fresh starts this decoder is for.
    key: (String, u64),
    player: Player,
    /// What the views were last told of it.
    told: Status,
}

/// How well the picture has kept to the song lately, for the log.
#[derive(Default)]
struct Kept {
    since: Option<Instant>,
    pictures: u32,
    /// The window's count of frames when the tally began.
    frames: Option<u64>,
    /// How far behind the song the pictures were when shown, in 100 ns.
    behind: i64,
    worst: i64,
}

impl Kept {
    fn note(&mut self, now: Instant, frames: u64, picture: i64, song: i64) {
        let from = *self.frames.get_or_insert(frames);
        let behind = song - picture;
        self.pictures += 1;
        self.behind += behind;
        self.worst = self.worst.max(behind);
        let since = *self.since.get_or_insert(now);
        if now.duration_since(since) < Duration::from_secs(1) {
            return;
        }
        log::debug!(
            "song at {} ms, picture at {} ms; {} pictures in {} frames, {:.1} ms behind on average, {:.1} at most",
            song / PER_MS,
            picture / PER_MS,
            self.pictures,
            frames - from,
            self.behind as f64 / f64::from(self.pictures) / PER_MS as f64,
            self.worst as f64 / PER_MS as f64,
        );
        *self = Self::default();
    }
}

impl App {
    /// Called once the views have drawn, and so said whether a surface for
    /// the picture is on screen.
    pub(super) fn show_video(&mut self, ctx: &egui::Context) {
        let watched = self.state.video.watched.take();
        let playback = self.state.playback.as_ref();
        let key = self.state.video.key(playback.and_then(Playback::current));
        let screen = &mut self.video;
        if screen.running.as_ref().map(|running| &running.key) != key.as_ref() {
            screen.running = None;
            screen.texture = None;
            self.state.video.picture = None;
        }
        let (Some(key), Some(playback)) = (key, playback) else {
            return;
        };
        let now = Instant::now();
        // A window in the tray draws nothing anybody sees.
        let seen = self.window_shown || self.state.mini_player;
        let Some(height) = watched.filter(|_| seen) else {
            let long_unwatched = screen
                .watched
                .is_none_or(|watched| now.duration_since(watched) > LET_GO_AFTER);
            if long_unwatched && screen.running.take().is_some() {
                screen.texture = None;
                self.state.video.picture = None;
            }
            return;
        };
        screen.watched = Some(now);
        // One that was let go while nobody looked is started again; the
        // last picture stays on screen until the first new one.
        if screen
            .running
            .as_ref()
            .is_some_and(|running| running.player.status() == Status::Asleep)
        {
            screen.running = None;
        }
        if screen.running.is_none() {
            let CoreStatus::Ready { origin } = &self.state.core else {
                return;
            };
            let address = Client::video_stream_url(origin, &key.0);
            let wake = {
                let ctx = ctx.clone();
                move || ctx.request_repaint()
            };
            screen.running = Some(Running {
                key,
                player: Player::start(address, wake),
                told: Status::Loading,
            });
        }
        let Some(running) = &mut screen.running else {
            return;
        };
        let clock = Clock {
            position: playback.position_ms(now) as i64 * PER_MS,
            playing: playback.is_playing(),
            speed: playback.speed,
            read: now,
        };
        running.player.tell(clock, height);
        let due = clock.position + if clock.playing { EARLY } else { 0 };
        if let Some(picture) = running.player.take(due) {
            screen
                .kept
                .note(now, self.frames, picture.time, clock.position);
            let size = vec2(picture.size[0] as f32, picture.size[1] as f32);
            let image = ColorImage {
                size: picture.size,
                source_size: size,
                pixels: picture.pixels,
            };
            // One texture, painted over: the pictures are all one size, and
            // a new texture for each would be made and thrown away 25 times
            // a second.
            let texture = match &mut screen.texture {
                Some(texture) => {
                    texture.set(image, TextureOptions::LINEAR);
                    texture.id()
                }
                None => {
                    let texture = ctx.load_texture("music-video", image, TextureOptions::LINEAR);
                    let id = texture.id();
                    screen.texture = Some(texture);
                    id
                }
            };
            self.state.video.picture = Some((texture, size));
        }
        let status = running.player.status();
        if status != running.told {
            match &status {
                Status::Showing => self.actions.push(Action::Video(VideoAsk::Loading(false))),
                Status::Failed(_) => self.actions.push(Action::Video(VideoAsk::Failed)),
                Status::Loading | Status::Asleep => {}
            }
            running.told = status;
        }
        // Drawn again when the next picture is due, and only while the
        // song moves: a paused picture is redrawn by whatever else happens,
        // and the decoder wakes the window itself for the first one.
        if clock.playing && running.told == Status::Showing {
            let wait = running
                .player
                .until_next(clock.position, clock.speed)
                .unwrap_or(LOOK_AGAIN);
            // egui wakes the window one frame's time before the delay it
            // is asked for, to be sure of not being late. Left at that,
            // each picture was woken for too soon and then waited for by
            // drawing, four or five frames to a picture; with the frame's
            // time put back, it is one.
            let frame = ctx.input(|input| input.predicted_dt);
            let frame = Duration::try_from_secs_f32(frame).unwrap_or_default();
            ctx.request_repaint_after(wait + frame);
        }
    }
}

//! What the worker keeps an eye on while music plays: the sound device,
//! the preload, and what the core is owed a report of.

use std::time::Instant;

use super::super::{Event, gain_for};
use super::{DEVICE_CHECK, POSITION_INTERVAL, Playing, STALL_AFTER, Worker};
use crate::deck::DeckError;
use crate::source::StreamError;

impl Worker {
    /// The device the app last asked for; `None` for the system's.
    pub(super) fn wanted_device(&self) -> Option<String> {
        self.dials.device.lock().ok().and_then(|id| id.clone())
    }

    /// Moves to another sound device when the one in use has gone or is no
    /// longer the one to play through: another was chosen, the chosen one
    /// came back, or the system's own choice moved. The decks resample to
    /// the device's rate, so they are opened again where the music had got
    /// to.
    pub(super) fn follow_device(&mut self) {
        let wanted = self.wanted_device();
        let chosen_anew = wanted != self.device;
        self.device = wanted;
        // A choice is acted on at once; what the system does by itself is
        // looked for now and then, and only while there is music to move.
        let due = self.target.playing && self.device_checked.elapsed() >= DEVICE_CHECK;
        if !chosen_anew && !due {
            return;
        }
        self.device_checked = Instant::now();
        let wanted = self.device.as_deref();
        if !self
            .output
            .as_ref()
            .is_some_and(|output| output.stale(wanted))
        {
            return;
        }
        log::info!("the sound device changed; moving to the new one");
        let position_ms = self.position_ms();
        let current = self
            .current
            .as_ref()
            .map(|playing| playing.deck.video_id().to_owned());
        let next = self.next.as_ref().map(|deck| deck.video_id().to_owned());
        self.output = None;
        self.equalizer = None;
        self.fading_out = None;
        self.current = current.and_then(|id| {
            self.spawn_deck(&id, position_ms, false)
                .map(|deck| Playing::new(deck, position_ms))
        });
        self.next = next.and_then(|id| self.spawn_deck(&id, 0, true));
        self.restart_watches();
        if let Some(output) = &self.output {
            output.set_gain(gain_for(self.target.volume));
            // A device chosen while paused is opened and left quiet.
            if self.target.playing {
                output.play();
            }
        }
    }

    /// A preload that fails is dropped quietly. It never fails the track
    /// that is playing; the next track loads the ordinary way when its
    /// turn comes.
    pub(super) fn watch_preload(&mut self) {
        // Its audio is left where it is, waiting to be promoted.
        let Some(deck) = &self.next else {
            return;
        };
        if deck.has_failed() {
            log::debug!("preload of {} failed", deck.video_id());
            self.failed_preload = Some(deck.video_id().to_owned());
            self.next = None;
        }
    }

    /// Writes where the track has reached for the picture to keep time
    /// with. A track still loading, or one waiting for audio, is not moving.
    pub(super) fn tell_clock(&self) {
        let (id, moving) = match &self.current {
            Some(playing) => (
                playing.deck.video_id(),
                self.target.playing && playing.ready && !playing.ended,
            ),
            None => ("", false),
        };
        self.clock.set(id, self.position_ms(), moving, self.speed);
    }

    pub(super) fn report(&mut self) {
        if !self.target.playing || !self.on_target() {
            return;
        }
        let Some(playing) = &self.current else {
            return;
        };
        if !playing.ready || playing.ended {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_audio) >= STALL_AFTER {
            if !self.stall_reported {
                self.stall_reported = true;
                (self.emit)(Event::Stalled {
                    epoch: self.target.epoch,
                });
            }
            // No position while frozen: each one would tell the core the
            // track is playing.
            return;
        }
        if now.duration_since(self.last_position_report) >= POSITION_INTERVAL {
            self.last_position_report = now;
            (self.emit)(Event::Position {
                epoch: self.target.epoch,
                position_ms: self.position_ms(),
                duration_ms: playing.duration_ms,
            });
        }
    }

    /// Reports that the current track cannot play, once per epoch.
    pub(super) fn fail(&mut self, error: DeckError) {
        self.current = None;
        let epoch = self.target.epoch;
        if self.failed_epoch == Some(epoch) {
            return;
        }
        self.failed_epoch = Some(epoch);
        log::warn!("track {} failed: {error:?}", self.target.video_id);
        (self.emit)(match error {
            DeckError::Stream(StreamError::RateLimited) => Event::Blocked { epoch },
            DeckError::Stream(StreamError::Offline) => Event::Stalled { epoch },
            DeckError::Stream(StreamError::Network(_)) => Event::Failed {
                epoch,
                reason: "network".into(),
            },
            DeckError::Stream(StreamError::Unplayable(_)) | DeckError::Decode(_) => Event::Failed {
                epoch,
                reason: String::new(),
            },
        });
    }
}

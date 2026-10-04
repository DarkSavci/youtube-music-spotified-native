//! The deck that is being heard, and one fading out under it.

use crate::deck::{Deck, Poll};
use crate::silence::Tail;
use crate::stretch::Stretcher;

/// The deck that is playing, and how far along it is.
pub(super) struct Playing {
    pub(super) deck: Deck,
    pub(super) ready: bool,
    pub(super) duration_ms: u64,
    /// Where the audio handed to the device so far reaches.
    pub(super) fed_until_ms: u64,
    /// A chunk partly handed over, and how much of it has been.
    pub(super) pending: Option<(Vec<f32>, usize)>,
    pub(super) ended: bool,
    /// Where the music ends, once a crossfade has wanted to know.
    pub(super) tail: Option<Tail>,
    /// Frames of silence still to be passed over at the start: set for a
    /// track that is faded into, and spent at its first sound.
    pub(super) quiet_to_skip: u64,
    /// Changes the speed of what the deck decodes, on its way to `pending`.
    stretch: Stretcher,
}

impl Playing {
    pub(super) fn new(deck: Deck, start_ms: u64) -> Self {
        Self {
            deck,
            ready: false,
            duration_ms: 0,
            fed_until_ms: start_ms,
            pending: None,
            ended: false,
            tail: None,
            quiet_to_skip: 0,
            stretch: Stretcher::default(),
        }
    }

    /// A deck that is faded into: up to `quiet_frames` of silence at its
    /// start are passed over.
    pub(super) fn faded_into(deck: Deck, quiet_frames: u64) -> Self {
        Self {
            quiet_to_skip: quiet_frames,
            ..Self::new(deck, 0)
        }
    }

    /// Makes a decoded chunk what is to be handed over next, from `skip`
    /// samples in, at `speed` times normal. At a speed other than one the
    /// chunk may be held back whole, to be heard with the next.
    pub(super) fn take(&mut self, samples: Vec<f32>, skip: usize, rate: u32, speed: f32) {
        if speed == 1.0 && self.stretch.is_idle() {
            self.pending = Some((samples, skip));
            return;
        }
        let mut stretched = Vec::with_capacity(samples.len());
        self.stretch
            .process(rate, speed, &samples[skip..], &mut stretched);
        self.pending = (!stretched.is_empty()).then_some((stretched, 0));
    }

    /// The deck has no more: what was held back for a later chunk is to be
    /// handed over as it is.
    pub(super) fn take_rest(&mut self) {
        let mut rest = Vec::new();
        self.stretch.drain(&mut rest);
        self.pending = (!rest.is_empty()).then_some((rest, 0));
    }

    /// How much of the track, in milliseconds, has been decoded and not yet
    /// handed on: what is held back to change its speed.
    pub(super) fn held_ms(&self, rate: u32) -> u64 {
        self.stretch.held_frames() as u64 * 1000 / u64::from(rate.max(1))
    }

    /// Forgets what was decoded before a seek.
    pub(super) fn clear(&mut self) {
        self.pending = None;
        self.stretch.reset();
    }

    /// The next frame of this deck, or silence if it has none ready. For a
    /// deck that is fading out under another.
    fn next_frame(&mut self, rate: u32, speed: f32) -> [f32; 2] {
        loop {
            if let Some((samples, sent)) = &mut self.pending {
                if *sent + 1 < samples.len() {
                    let frame = [samples[*sent], samples[*sent + 1]];
                    *sent += 2;
                    return frame;
                }
                self.pending = None;
            }
            match self.deck.poll() {
                Poll::Chunk { samples, .. } => self.take(samples, 0, rate, speed),
                Poll::Ended if !self.ended => {
                    self.ended = true;
                    self.take_rest();
                }
                _ => return [0.0; 2],
            }
        }
    }
}

/// One track fading out under the next. Both follow equal-power curves, so
/// the loudness holds steady through the middle of the fade.
pub(super) struct Fade {
    pub(super) outgoing: Playing,
    /// The fade's length and how far along it is, in frames.
    pub(super) total: u64,
    pub(super) done: u64,
}

impl Fade {
    /// The gains for the incoming and the outgoing track at this point.
    fn gains(&self) -> (f32, f32) {
        let progress = (self.done as f32 / self.total.max(1) as f32).min(1.0);
        let angle = progress * std::f32::consts::FRAC_PI_2;
        (angle.sin(), angle.cos())
    }

    /// Fades `incoming` in, in place, with the outgoing track under it.
    pub(super) fn mix_into(&mut self, incoming: &mut [f32], rate: u32, speed: f32) {
        for frame in incoming.as_chunks_mut::<2>().0 {
            let (rising, falling) = self.gains();
            let old = self.outgoing.next_frame(rate, speed);
            frame[0] = frame[0] * rising + old[0] * falling;
            frame[1] = frame[1] * rising + old[1] * falling;
            self.done += 1;
        }
    }

    pub(super) fn finished(&self) -> bool {
        self.done >= self.total
    }
}

//! What `--open` asks for once the app has settled, one step at a time:
//! things to do, pauses between them, and pictures along the way. It is
//! how a room is driven through its paces without a hand on the mouse.

use std::collections::VecDeque;
use std::time::Instant;

use eframe::egui;

use super::App;
use super::open::{self, Step};
use crate::screenshot;

/// The steps still to come, and when the next may be taken.
#[derive(Default)]
pub(super) struct Script {
    steps: VecDeque<String>,
    /// The app has settled once: what follows goes by the clock alone.
    begun: bool,
    /// The pause a `wait:` asked for is over at this moment.
    resumes: Option<Instant>,
    /// A picture has been asked for and has yet to arrive. The steps after
    /// it wait, or it would be a picture of what they did.
    picturing: bool,
    /// What the pointer and the keys are to do on the next frame.
    input: Vec<egui::Event>,
}

impl Script {
    pub(super) fn new(steps: impl IntoIterator<Item = String>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Hands the window what a step asked of the pointer and the keys.
    pub(super) fn feed(&mut self, raw_input: &mut egui::RawInput) {
        raw_input.events.append(&mut self.input);
    }

    /// Whether every step has been taken.
    pub(super) fn is_done(&self) -> bool {
        self.steps.is_empty() && self.resumes.is_none() && !self.picturing
    }
}

impl App {
    /// Takes the steps that are due. `settled` says the player is up and
    /// whatever was asked to play has started, which the first step waits
    /// for: a room is entered with the music that is playing.
    pub(super) fn run_script(&mut self, ctx: &egui::Context, settled: bool) {
        if screenshot::save_marked(ctx) {
            self.script.picturing = false;
        }
        if self.script.is_done() {
            return;
        }
        if self.script.picturing {
            ctx.request_repaint();
            return;
        }
        let now = Instant::now();
        if !self.script.begun {
            if !settled || self.requests_in_flight > 0 {
                return;
            }
            self.script.begun = true;
        }
        if let Some(resumes) = self.script.resumes {
            if now < resumes {
                ctx.request_repaint_after(resumes - now);
                return;
            }
            self.script.resumes = None;
        }
        while let Some(spec) = self.script.steps.pop_front() {
            match open::step(&spec, &self.state) {
                Step::Do(actions) => self.actions.extend(actions),
                Step::Flyout => self.open_flyout(ctx),
                Step::Input(events) => {
                    // Seen by the next frame, which the step after waits for.
                    self.script.input.extend(events);
                    break;
                }
                Step::Wait(pause) => {
                    self.script.resumes = Some(now + pause);
                    ctx.request_repaint_after(pause);
                    break;
                }
                Step::Shot(path) => {
                    screenshot::ask_marked(ctx, path);
                    self.script.picturing = true;
                    break;
                }
            }
        }
        ctx.request_repaint();
    }
}

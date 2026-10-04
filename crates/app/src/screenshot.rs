//! `--screenshot`: save the window to a PNG and exit.
//!
//! Used with `--demo` for pictures that are the same on every run and show
//! no account data, to compare one change with the next.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

/// Frames drawn after the content has settled, so fonts and icons that load
/// on first use are on screen.
const SETTLE_FRAMES: u32 = 5;
/// Take the picture anyway if the content never settles.
const GIVE_UP_AFTER: Duration = Duration::from_secs(8);

pub struct Screenshot {
    path: PathBuf,
    began: Instant,
    settled_frames: u32,
    asked: bool,
}

impl Screenshot {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            began: Instant::now(),
            settled_frames: 0,
            asked: false,
        }
    }

    /// Call once a frame. `settled` says the window shows what it will show.
    pub fn step(&mut self, ctx: &egui::Context, settled: bool) {
        // Nothing here is driven by input, so the frames have to be asked for.
        ctx.request_repaint();

        if settled || self.began.elapsed() >= GIVE_UP_AFTER {
            self.settled_frames += 1;
        }
        if !self.asked && self.settled_frames >= SETTLE_FRAMES {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.asked = true;
        }

        let image = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        let Some(image) = image else {
            return;
        };
        let [width, height] = image.size;
        let result = image::save_buffer(
            &self.path,
            image.as_raw(),
            width as u32,
            height as u32,
            image::ColorType::Rgba8,
        );
        match result {
            Ok(()) => log::info!("screenshot saved to {}", self.path.display()),
            Err(error) => log::error!("screenshot could not be saved: {error}"),
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

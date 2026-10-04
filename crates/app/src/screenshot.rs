//! `--screenshot`: save the window to a PNG and exit.
//!
//! Used with `--demo` for pictures that are the same on every run and show
//! no account data, to compare one change with the next.

use std::path::{Path, PathBuf};
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

    /// Call once a frame. `settled` says the window shows what it will
    /// show; `scripted` that `--open` still has steps to take, which the
    /// picture waits for however long they are.
    pub fn step(&mut self, ctx: &egui::Context, settled: bool, scripted: bool) {
        // Nothing here is driven by input, so the frames have to be asked for.
        ctx.request_repaint();

        if scripted {
            self.began = Instant::now();
        }
        if settled || self.began.elapsed() >= GIVE_UP_AFTER {
            self.settled_frames += 1;
        }
        if !self.asked && self.settled_frames >= SETTLE_FRAMES {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.asked = true;
        }

        let image = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                // A picture asked for along the way is not the last one.
                egui::Event::Screenshot {
                    image, user_data, ..
                } if user_data.data.is_none() => Some(image.clone()),
                _ => None,
            })
        });
        let Some(image) = image else {
            return;
        };
        save(&self.path, &image);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

fn save(path: &Path, image: &egui::ColorImage) {
    let [width, height] = image.size;
    let result = image::save_buffer(
        path,
        image.as_raw(),
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    );
    match result {
        Ok(()) => log::info!("screenshot saved to {}", path.display()),
        Err(error) => log::error!("screenshot could not be saved: {error}"),
    }
}

/// Asks for a picture of the window as it is, to be saved at `path`. The
/// app carries on: this is `--open shot:`, a picture along the way.
pub fn ask_marked(ctx: &egui::Context, path: PathBuf) {
    let marked = egui::UserData::new(path);
    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(marked));
}

/// Saves the pictures [`ask_marked`] asked for that have arrived, and says
/// whether any had.
pub fn save_marked(ctx: &egui::Context) -> bool {
    let arrived: Vec<(PathBuf, std::sync::Arc<egui::ColorImage>)> = ctx.input(|input| {
        let pictures = input.events.iter().filter_map(|event| match event {
            egui::Event::Screenshot {
                image, user_data, ..
            } => {
                let path = user_data.data.as_ref()?.downcast_ref::<PathBuf>()?;
                Some((path.clone(), image.clone()))
            }
            _ => None,
        });
        pictures.collect()
    });
    for (path, image) in &arrived {
        save(path, image);
    }
    !arrived.is_empty()
}

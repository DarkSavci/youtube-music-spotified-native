//! Artwork: fetched and decoded off the UI thread, kept within a budget.
//!
//! A view asks for a URL's texture every frame it draws it. The first ask
//! queues a load; until that lands the view draws a placeholder. Textures
//! not drawn lately are dropped once the total passes the budget, so memory
//! stays flat however much is browsed.
//!
//! Views hold `&State`, so the asks are recorded through interior
//! mutability. It is the one place a view leaves a mark, and all it records
//! is "this was wanted".

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossbeam_channel::Sender;
use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureId, TextureOptions};

use crate::tint;

/// Decoded pixels held as textures before the least recently drawn go.
const BUDGET_BYTES: usize = 64 * 1024 * 1024;
/// A texture drawn this recently is on screen or about to be again.
const KEEP_FRAMES: u64 = 120;
/// No cover needs more than this, and nothing larger is worth decoding.
const MAX_DOWNLOAD_BYTES: u64 = 8 * 1024 * 1024;
const WORKERS: usize = 4;
const TIMEOUT: Duration = Duration::from_secs(20);

enum Slot {
    Loading,
    Ready(TextureHandle),
    /// Not retried: a URL that failed once fails the same way next frame.
    Failed,
}

struct Entry {
    slot: Slot,
    last_drawn: Cell<u64>,
    /// The colour the cover lends a page; `None` for a grey one.
    tint: Option<Color32>,
}

#[derive(Default)]
pub struct Images {
    entries: HashMap<String, Entry>,
    /// URLs asked for this frame that nothing is known about yet.
    wanted: RefCell<Vec<String>>,
    frame: u64,
}

impl Images {
    /// The texture for `url` and its size in pixels, if it has loaded.
    /// Asking is what loads it.
    pub fn texture(&self, url: &str) -> Option<(TextureId, egui::Vec2)> {
        match self.entries.get(url) {
            Some(entry) => {
                entry.last_drawn.set(self.frame);
                match &entry.slot {
                    Slot::Ready(texture) => Some((texture.id(), texture.size_vec2())),
                    Slot::Loading | Slot::Failed => None,
                }
            }
            None => {
                self.wanted.borrow_mut().push(url.to_owned());
                None
            }
        }
    }

    /// The texture for `url` if it has already loaded, without asking for
    /// it: for a picture that would only stand in for another.
    pub fn ready(&self, url: &str) -> Option<(TextureId, egui::Vec2)> {
        let entry = self.entries.get(url)?;
        let Slot::Ready(texture) = &entry.slot else {
            return None;
        };
        entry.last_drawn.set(self.frame);
        Some((texture.id(), texture.size_vec2()))
    }

    /// The tint of the cover at `url`, once it has loaded. Asking does not
    /// load it: a tint follows a cover that is on screen anyway.
    pub fn tint(&self, url: &str) -> Option<Color32> {
        self.entries.get(url).and_then(|entry| entry.tint)
    }

    /// Whether anything is still on its way, for `--screenshot` to wait on.
    pub fn loading(&self) -> bool {
        self.entries
            .values()
            .any(|entry| matches!(entry.slot, Slot::Loading))
    }

    /// Once a frame, after drawing: start what was newly asked for and drop
    /// what has gone unseen while over budget.
    pub fn end_frame(&mut self, loader: &ImageLoader) {
        for url in self.wanted.take() {
            if !self.entries.contains_key(&url) {
                loader.load(url.clone());
                self.entries.insert(
                    url,
                    Entry {
                        slot: Slot::Loading,
                        last_drawn: Cell::new(self.frame),
                        tint: None,
                    },
                );
            }
        }
        self.frame += 1;
        self.evict();
    }

    pub fn loaded(&mut self, ctx: &egui::Context, url: String, image: Option<ColorImage>) {
        // Evicted or never asked for: nobody is waiting.
        let Some(entry) = self.entries.get_mut(&url) else {
            return;
        };
        entry.slot = match image {
            Some(image) => {
                entry.tint = tint::of(&image);
                Slot::Ready(ctx.load_texture(&url, image, TextureOptions::LINEAR))
            }
            None => Slot::Failed,
        };
    }

    fn evict(&mut self) {
        let mut total: usize = self.entries.values().map(texture_bytes).sum();
        if total <= BUDGET_BYTES {
            return;
        }
        let stale_before = self.frame.saturating_sub(KEEP_FRAMES);
        let mut stale: Vec<(u64, String, usize)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.last_drawn.get() < stale_before)
            .map(|(url, entry)| (entry.last_drawn.get(), url.clone(), texture_bytes(entry)))
            .collect();
        stale.sort();
        for (_, url, bytes) in stale {
            if total <= BUDGET_BYTES {
                break;
            }
            self.entries.remove(&url);
            total -= bytes;
        }
    }
}

fn texture_bytes(entry: &Entry) -> usize {
    match &entry.slot {
        Slot::Ready(texture) => texture.byte_size(),
        Slot::Loading | Slot::Failed => 0,
    }
}

/// Worker threads that turn a URL into pixels: from the disk cache when the
/// image has been seen before, from the network otherwise.
pub struct ImageLoader {
    jobs: Sender<String>,
}

impl ImageLoader {
    /// `deliver` is called on a worker thread with each result; `None`
    /// means the image could not be had.
    pub fn start(
        cache_dir: PathBuf,
        deliver: impl Fn(String, Option<ColorImage>) + Send + Clone + 'static,
    ) -> Self {
        let (jobs, queue) = crossbeam_channel::unbounded::<String>();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .provider(ureq::tls::TlsProvider::NativeTls)
                    // Trust what the system trusts, not a bundled list.
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .build()
            .new_agent();
        if let Err(error) = std::fs::create_dir_all(&cache_dir) {
            log::warn!("the artwork cache cannot be created: {error}");
        }
        for index in 0..WORKERS {
            let queue = queue.clone();
            let agent = agent.clone();
            let cache_dir = cache_dir.clone();
            let deliver = deliver.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("image-{index}"))
                .spawn(move || {
                    for url in queue {
                        let image = fetch(&agent, &cache_dir, &url);
                        deliver(url, image);
                    }
                });
            if let Err(error) = spawned {
                log::error!("an image worker could not start: {error}");
            }
        }
        Self { jobs }
    }

    fn load(&self, url: String) {
        let _ = self.jobs.send(url);
    }
}

fn fetch(agent: &ureq::Agent, cache_dir: &Path, url: &str) -> Option<ColorImage> {
    let cached = cache_dir.join(cache_name(url));
    if let Ok(bytes) = std::fs::read(&cached)
        && let Some(image) = decode(&bytes)
    {
        return Some(image);
    }
    let bytes = match download(agent, url) {
        Ok(bytes) => bytes,
        Err(error) => {
            log::debug!("artwork not fetched: {error}");
            return None;
        }
    };
    let image = decode(&bytes)?;
    // Written under another name and renamed, so a reader never finds half
    // a file under the real one.
    let partial = cached.with_extension("part");
    if std::fs::write(&partial, &bytes).is_ok() {
        let _ = std::fs::rename(&partial, &cached);
    }
    Some(image)
}

fn download(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    agent
        .get(url)
        .call()?
        .into_body()
        .into_reader()
        .take(MAX_DOWNLOAD_BYTES)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Option<ColorImage> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, image.as_raw()))
}

/// A file name for a URL: FNV-1a, which unlike the standard library's hasher
/// gives the same name on every run and every build.
fn cache_name(url: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_always_maps_to_the_same_file() {
        assert_eq!(cache_name(""), "cbf29ce484222325");
        assert_eq!(cache_name("a"), "af63dc4c8601ec8c");
        assert_ne!(cache_name("https://x/a=w60"), cache_name("https://x/a=w61"));
    }

    #[test]
    fn asking_for_an_unknown_url_queues_it_once_loaded() {
        let images = Images::default();
        assert_eq!(images.texture("https://x/a"), None);
        assert_eq!(images.wanted.borrow().as_slice(), ["https://x/a"]);
    }
}

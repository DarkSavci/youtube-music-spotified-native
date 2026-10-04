//! Where the app keeps its files.
//!
//! Deliberately not the Electron app's `Spotifier` directory: the two apps
//! must be able to run side by side without touching each other's data.

use std::io;
use std::path::{Path, PathBuf};

const APP_DIR: &str = "SpotifiedNative";

#[derive(Debug, Clone)]
pub struct Paths {
    /// Settings, credentials and the core's database (roaming).
    pub config: PathBuf,
    /// Artwork and other things that can be rebuilt (local).
    pub cache: PathBuf,
    pub logs: PathBuf,
}

impl Paths {
    /// The real profile; or everything under `profile` when one is named;
    /// or, for `--demo` without one, a throwaway under the temp directory,
    /// so a demo run can never read or write account data.
    pub fn discover(demo: bool, profile: Option<&Path>) -> io::Result<Self> {
        let paths = if let Some(root) = profile {
            Self::under(&root.join("config"), &root.join("local"))
        } else if demo {
            // One fixed place, emptied by `start_clean` once this copy is
            // known to be the only one using it.
            let root = std::env::temp_dir().join("spotified-demo");
            Self::under(&root.join("config"), &root.join("local"))
        } else {
            let base = directories::BaseDirs::new()
                .ok_or_else(|| io::Error::other("no home directory to keep settings in"))?;
            Self::under(
                &base.config_dir().join(APP_DIR),
                &base.data_local_dir().join(APP_DIR),
            )
        };
        for dir in [&paths.config, &paths.cache, &paths.logs] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(paths)
    }

    fn under(config: &Path, local: &Path) -> Self {
        Self {
            config: config.to_path_buf(),
            cache: local.join("cache"),
            logs: local.join("logs"),
        }
    }

    /// Empties the profile, so a demo always begins the same way and its
    /// log is still there to read afterwards. Called only with the instance
    /// lock held: emptying a profile another copy is running on would take
    /// its lock away with everything else.
    pub fn start_clean(&self) -> io::Result<()> {
        for dir in [&self.cache, &self.logs] {
            let _ = std::fs::remove_dir_all(dir);
            std::fs::create_dir_all(dir)?;
        }
        for entry in std::fs::read_dir(&self.config)?.flatten() {
            let held = entry.file_name().to_string_lossy().starts_with("instance.");
            if held {
                continue;
            }
            let path = entry.path();
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
        }
        Ok(())
    }

    /// Where theme files are looked for.
    pub fn themes_folder(&self) -> PathBuf {
        self.config.join("themes")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config.join("settings.json")
    }

    /// Where the songs played are kept, whoever is signed in.
    pub fn audio_cache(&self) -> PathBuf {
        self.config.join("audio-cache")
    }

    /// Where newer copies of yt-dlp are kept. Beside the cache, not in the
    /// roaming profile: they are large and can be fetched again.
    pub fn resolver_folder(&self) -> PathBuf {
        self.cache.with_file_name("yt-dlp")
    }

    pub fn instance_lock(&self) -> PathBuf {
        self.config.join("instance.lock")
    }
}

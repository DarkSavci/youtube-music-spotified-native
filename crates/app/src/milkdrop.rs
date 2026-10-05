//! MilkDrop: the music drawn by MilkDrop's presets, in a window of its own.
//!
//! The drawing is libprojectM's, which reads MilkDrop `.milk` presets and
//! renders them with OpenGL. It is not built into the app: it is a library
//! beside it (`libprojectM-4.dll`, built by `scripts/build-projectm.ps1`),
//! found and loaded when the window is first asked for, so a copy without
//! it simply has no MilkDrop. The window runs as this same program started
//! again with `--milkdrop-child`, because the window toolkit allows one
//! event loop to a process and the app's own has it. The sound reaches
//! that process through a ring of samples in a file both have mapped.
//!
//! No presets come with the app. The folder is the profile's `milkdrop`,
//! and Settings fetches the packs projectM curates into it.
//!
//! The arrangement follows Spotifast's `src/milkdrop` (MIT, see NOTICE.md).

#[cfg(windows)]
pub mod child;
pub mod host;
#[cfg(windows)]
mod library;
pub mod ring;

use std::path::{Path, PathBuf};

/// The library, as its build names it.
pub const LIBRARY: &str = "libprojectM-4.dll";
/// How long a preset plays before the next fades in.
pub const PRESET_SECONDS: u32 = 20;
/// How deep into the folder presets are looked for: packs come in a folder
/// of folders.
const MAX_DEPTH: usize = 4;

/// A pack of presets projectM curates, fetched as a zip of `.milk` files.
pub struct Pack {
    pub url: &'static str,
    /// What Settings calls it.
    pub label: &'static str,
}

pub const PACKS: [Pack; 2] = [
    // The 550 presets that shipped with MilkDrop 2; about a megabyte.
    Pack {
        url: "https://github.com/projectM-visualizer/presets-milkdrop-original/archive/refs/heads/master.zip",
        label: "Get presets",
    },
    // Jason Fletcher's pick of 9,800 the community made; about 25 MB.
    Pack {
        url: "https://github.com/projectM-visualizer/presets-cream-of-the-crop/archive/refs/heads/master.zip",
        label: "Get 9,800 more",
    },
];

/// What the app knows of MilkDrop, to show.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Status {
    /// Its window is open.
    pub open: bool,
    /// How many presets are in the folder.
    pub presets: usize,
    /// A pack is being fetched.
    pub fetching: bool,
}

/// What can be asked of MilkDrop.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// Open its window, or close it.
    Toggle,
    /// The window could not be opened.
    Failed(String),
    /// The window has gone: closed from inside it.
    Closed,
    /// Fetch one of [`PACKS`], by its place among them.
    GetPresets(usize),
    /// That ended: with how many presets were written, or with why none.
    Fetched(Result<usize, String>),
    OpenFolder,
}

/// Where the library is: beside the program when installed, or where the
/// build script leaves it in a working tree.
pub fn library() -> Option<PathBuf> {
    crate::sidecar::locate(LIBRARY)
        .or_else(|| crate::sidecar::locate(Path::new("../projectm/src/libprojectM").join(LIBRARY)))
}

/// Every `.milk` file in a folder and the folders inside it, sorted by
/// path without regard to case.
pub fn list_presets(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk(folder, 0, &mut files);
    files.sort_by_cached_key(|path| path.to_string_lossy().to_lowercase());
    files
}

fn walk(folder: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth + 1 < MAX_DEPTH {
                walk(&path, depth + 1, files);
            }
        } else if is_preset(&path) {
            files.push(path);
        }
    }
}

fn is_preset(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("milk"))
}

/// Downloads a pack and writes its presets into the folder. `scratch` is
/// where the download is kept while it is read. Returns how many presets
/// were written.
pub fn fetch_pack(pack: &Pack, folder: &Path, scratch: &Path) -> Result<usize, String> {
    let said = |error: &dyn std::fmt::Display| error.to_string();
    std::fs::create_dir_all(scratch).map_err(|error| said(&error))?;
    let zip = scratch.join("milkdrop-presets.zip");
    let agent = crate::update::agent();
    crate::update::download(&agent, pack.url, &zip).map_err(|error| said(&error))?;
    let bytes = std::fs::read(&zip).map_err(|error| said(&error));
    let _ = std::fs::remove_file(&zip);
    unpack_presets(&bytes?, folder)
}

/// Writes the `.milk` files of a zip into the folder, flat: the packs keep
/// theirs in folders by style, and the names do not clash.
pub fn unpack_presets(zip: &[u8], folder: &Path) -> Result<usize, String> {
    let archive = crate::skin::zip::Archive::parse(zip).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let mut written = 0;
    for entry in archive.entries() {
        if entry.is_dir() {
            continue;
        }
        let name = entry.base_name();
        if name.is_empty() || !is_preset(Path::new(name)) {
            continue;
        }
        let bytes = archive.read(entry).map_err(|error| error.to_string())?;
        std::fs::write(folder.join(name), bytes).map_err(|error| error.to_string())?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("spotified-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        dir
    }

    #[test]
    fn presets_are_found_in_folders_of_folders_by_name() {
        let dir = temp_dir("milkdrop-list");
        std::fs::create_dir_all(dir.join("Pack/Waves")).expect("folders");
        std::fs::write(dir.join("zebra.milk"), b"").expect("a preset");
        std::fs::write(dir.join("Pack/Waves/Alpha.MILK"), b"").expect("a preset");
        std::fs::write(dir.join("Pack/readme.txt"), b"").expect("a file");
        let names: Vec<String> = list_presets(&dir)
            .iter()
            .filter_map(|path| Some(path.file_name()?.to_string_lossy().into_owned()))
            .collect();
        std::fs::remove_dir_all(&dir).expect("removed");
        assert_eq!(names, ["Alpha.MILK", "zebra.milk"]);
        assert!(list_presets(Path::new("/nonexistent/milkdrop")).is_empty());
    }

    #[test]
    fn a_pack_is_unpacked_flat_and_only_its_presets() {
        let dir = temp_dir("milkdrop-unpack");
        let zip = crate::skin::zip::write(&[
            ("pack-master/", b"", false),
            ("pack-master/Dancer/one.milk", b"[preset00]", true),
            ("pack-master/Fractal/two.milk", b"[preset00]", false),
            ("pack-master/README.md", b"about", false),
        ]);
        assert_eq!(unpack_presets(&zip, &dir), Ok(2));
        assert!(dir.join("one.milk").is_file());
        assert!(dir.join("two.milk").is_file());
        assert!(!dir.join("README.md").exists());
        assert!(unpack_presets(b"not a zip", &dir).is_err());
        std::fs::remove_dir_all(&dir).expect("removed");
    }
}

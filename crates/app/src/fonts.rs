//! The fonts behind Inter, for what Inter cannot write.
//!
//! Inter has Latin, Greek and Cyrillic. A song's title is in whatever its
//! artist writes in, and a character no font here has is drawn as an empty
//! box. So Windows' own fonts stand behind the bundled ones: Segoe UI for
//! Arabic and Hebrew, its symbols, Yu Gothic for Japanese, YaHei for
//! Chinese, Malgun Gothic for Korean, Leelawadee for Thai, Nirmala for the
//! scripts of India, and Segoe's emoji last, as outlines.
//!
//! They are mapped, not read: between them they are some seventy megabytes
//! of files, of which a title in Japanese needs a few kilobytes. Mapped,
//! Windows brings in the pages a glyph is on when it is first drawn and
//! shares them with every other program using the font, so they cost the
//! start-up nothing to speak of and the app's own memory nearly nothing.
//! A font that is not on this computer is passed over without a word.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Which of Inter's weights a face stands behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    /// Behind the regular and the medium.
    Regular,
    /// Behind the semibold and the bold.
    Bold,
}

/// A font file of the system's, mapped for as long as the app runs.
pub struct Face {
    /// What egui knows it by.
    pub name: String,
    pub bytes: &'static [u8],
    /// Which face of a collection.
    pub index: u32,
    pub weight: Weight,
    /// How far down its glyphs are moved, as a part of the font's size.
    pub drop: f32,
}

/// File, face within it, the weight it stands behind, and how far down
/// its glyphs are moved, as a part of the font's size; in the order they
/// are asked for a glyph. Japanese comes before Chinese: the two share
/// most of their characters, and one of them has to draw them. Yu Gothic
/// sets its characters higher in its line than Inter does, so they are
/// brought down to sit on Inter's baseline.
const FACES: &[(&str, u32, Weight, f32)] = &[
    ("segoeui.ttf", 0, Weight::Regular, 0.0),
    ("seguisb.ttf", 0, Weight::Bold, 0.0),
    ("seguisym.ttf", 0, Weight::Regular, 0.0),
    ("seguisym.ttf", 0, Weight::Bold, 0.0),
    ("YuGothR.ttc", 0, Weight::Regular, YU_GOTHIC_DROP),
    ("YuGothB.ttc", 0, Weight::Bold, YU_GOTHIC_DROP),
    ("msyh.ttc", 0, Weight::Regular, 0.0),
    ("msyhbd.ttc", 0, Weight::Bold, 0.0),
    ("malgun.ttf", 0, Weight::Regular, 0.0),
    ("malgunbd.ttf", 0, Weight::Bold, 0.0),
    ("LeelawUI.ttf", 0, Weight::Regular, 0.0),
    ("LeelaUIb.ttf", 0, Weight::Bold, 0.0),
    ("Nirmala.ttc", 0, Weight::Regular, 0.0),
    ("Nirmala.ttc", 0, Weight::Bold, 0.0),
    ("seguiemj.ttf", 0, Weight::Regular, 0.0),
    ("seguiemj.ttf", 0, Weight::Bold, 0.0),
];

const YU_GOTHIC_DROP: f32 = 0.30;

/// The system's fonts that stand behind the bundled ones. Mapped once,
/// however often the fonts are set.
pub fn system() -> &'static [Face] {
    static FACES_FOUND: OnceLock<Vec<Face>> = OnceLock::new();
    FACES_FOUND.get_or_init(|| {
        // A test draws the interface, not the world's scripts, and there
        // are hundreds of them.
        if cfg!(test) {
            return Vec::new();
        }
        system_folder()
            .map(|folder| load(&folder))
            .unwrap_or_default()
    })
}

/// Where Windows keeps its fonts.
fn system_folder() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let windows = std::env::var_os("SystemRoot").or_else(|| std::env::var_os("windir"))?;
    Some(Path::new(&windows).join("Fonts"))
}

/// Maps every font of the list that `folder` has.
pub fn load(folder: &Path) -> Vec<Face> {
    // A file standing behind two weights is mapped once.
    let mut mapped: Vec<(&str, &'static [u8])> = Vec::new();
    let mut faces = Vec::new();
    for &(file, index, weight, drop) in FACES {
        let known = mapped.iter().find(|(name, _)| *name == file);
        let bytes = match known {
            Some((_, bytes)) => *bytes,
            None => {
                let Some(bytes) = map::file(&folder.join(file)) else {
                    continue;
                };
                mapped.push((file, bytes));
                bytes
            }
        };
        faces.push(Face {
            name: format!("system-{file}-{weight:?}"),
            bytes,
            index,
            weight,
            drop,
        });
    }
    faces
}

#[cfg(windows)]
mod map {
    use std::fs::File;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;

    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_READ, MapViewOfFile, PAGE_READONLY,
    };

    /// The file's bytes, mapped read-only and never unmapped: a font is
    /// in use until the app closes. `None` when it is not there, is
    /// empty, or cannot be mapped.
    pub fn file(path: &Path) -> Option<&'static [u8]> {
        let file = File::open(path).ok()?;
        let length = usize::try_from(file.metadata().ok()?.len()).ok()?;
        if length == 0 {
            return None;
        }
        let handle = HANDLE(file.as_raw_handle());
        // SAFETY: `handle` is the open file's, valid for the call; the
        // mapping keeps the file open by itself once made.
        let mapping =
            unsafe { CreateFileMappingW(handle, None, PAGE_READONLY, 0, 0, None) }.ok()?;
        // SAFETY: `mapping` was just made and is a valid mapping handle.
        let view = unsafe { MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0) };
        // The view holds the mapping; the handle is not needed again.
        // SAFETY: `mapping` is a handle this function owns.
        let _ = unsafe { CloseHandle(mapping) };
        if view.Value.is_null() {
            return None;
        }
        // SAFETY: the view is `length` bytes of the file, readable, and
        // is never unmapped, so the slice is good for the rest of the
        // run. A system font is not rewritten while Windows is up.
        Some(unsafe { std::slice::from_raw_parts(view.Value.cast::<u8>(), length) })
    }
}

#[cfg(not(windows))]
mod map {
    use std::path::Path;

    /// The system's fonts are Windows'; elsewhere egui's own stand alone.
    pub fn file(_path: &Path) -> Option<&'static [u8]> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_with_no_fonts_gives_none_and_says_nothing() {
        let nowhere = std::env::temp_dir().join("spotified-no-fonts-here");
        assert!(load(&nowhere).is_empty());
    }

    #[test]
    fn a_file_that_is_empty_or_not_there_is_not_a_font() {
        let folder = std::env::temp_dir().join(format!("spotified-fonts-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a folder");
        std::fs::write(folder.join("segoeui.ttf"), b"").expect("an empty file");
        assert!(load(&folder).is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[cfg(windows)]
    #[test]
    fn the_fonts_this_computer_has_are_mapped_whole() {
        let Some(folder) = system_folder() else {
            return;
        };
        for face in load(&folder) {
            // A TrueType file or a collection of them, by its first bytes.
            let tag = &face.bytes[..4];
            let known = [&b"ttcf"[..], &[0, 1, 0, 0], b"OTTO", b"true"];
            assert!(known.contains(&tag), "{} is not a font", face.name);
        }
    }
}

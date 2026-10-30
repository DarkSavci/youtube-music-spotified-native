//! The fonts behind Inter, for what Inter cannot write.
//!
//! Inter has Latin, Greek and Cyrillic. A song's title is in whatever its
//! artist writes in, and a character no font here has is drawn as an empty
//! box. So the system's own fonts stand behind the bundled ones. On Windows:
//! Segoe UI for Arabic and Hebrew, its symbols, Yu Gothic for Japanese, YaHei
//! for Chinese, Malgun Gothic for Korean, Leelawadee for Thai, Nirmala for
//! the scripts of India, and Segoe's emoji last, as outlines. A Mac and a
//! Linux desktop have their own for the same scripts; Apple's emoji are
//! bitmaps, which egui cannot draw, so its own emoji stay there.
//!
//! They are mapped, not read: between them they are some seventy megabytes
//! of files, of which a title in Japanese needs a few kilobytes. Mapped, the
//! system brings in the pages a glyph is on when it is first drawn and
//! shares them with every other program using the font, so they cost the
//! start-up nothing to speak of and the app's own memory nearly nothing.
//! A font that is not on this computer is passed over without a word.

#[cfg(not(target_os = "linux"))]
use std::path::Path;
use std::path::PathBuf;
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
#[cfg(windows)]
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

#[cfg(windows)]
const YU_GOTHIC_DROP: f32 = 0.30;

/// A Mac's, under `/System/Library/Fonts`. In the same order, for the same
/// reason, as Windows'.
#[cfg(target_os = "macos")]
const FACES: &[(&str, u32, Weight, f32)] = &[
    ("Supplemental/GeezaPro.ttc", 0, Weight::Regular, 0.0),
    ("Supplemental/GeezaPro.ttc", 1, Weight::Bold, 0.0),
    ("Supplemental/ArialHB.ttc", 0, Weight::Regular, 0.0),
    ("Supplemental/ArialHB.ttc", 1, Weight::Bold, 0.0),
    ("Apple Symbols.ttf", 0, Weight::Regular, 0.0),
    ("Apple Symbols.ttf", 0, Weight::Bold, 0.0),
    ("ヒラギノ角ゴシック W3.ttc", 0, Weight::Regular, 0.0),
    ("ヒラギノ角ゴシック W6.ttc", 0, Weight::Bold, 0.0),
    ("STHeiti Light.ttc", 0, Weight::Regular, 0.0),
    ("STHeiti Medium.ttc", 0, Weight::Bold, 0.0),
    ("AppleSDGothicNeo.ttc", 0, Weight::Regular, 0.0),
    ("AppleSDGothicNeo.ttc", 6, Weight::Bold, 0.0),
    ("Supplemental/Thonburi.ttc", 0, Weight::Regular, 0.0),
    ("Supplemental/Thonburi.ttc", 1, Weight::Bold, 0.0),
    ("Kohinoor.ttc", 0, Weight::Regular, 0.0),
    ("Kohinoor.ttc", 0, Weight::Bold, 0.0),
];

/// A Linux desktop's, by the names their files begin with: where a
/// distribution puts them differs, and what they are called mostly does not.
/// The first of a row that is found is the one used. Noto's Japanese comes
/// first in its collection.
#[cfg(target_os = "linux")]
const FAMILIES: &[(&[&str], Weight)] = &[
    (
        &["NotoSansArabic-Regular", "NotoSansArabic["],
        Weight::Regular,
    ),
    (&["NotoSansArabic-Bold", "NotoSansArabic["], Weight::Bold),
    (
        &["NotoSansHebrew-Regular", "NotoSansHebrew["],
        Weight::Regular,
    ),
    (&["NotoSansHebrew-Bold", "NotoSansHebrew["], Weight::Bold),
    (&["NotoSansSymbols2-Regular"], Weight::Regular),
    (&["NotoSansSymbols2-Regular"], Weight::Bold),
    (
        &["NotoSansSymbols-Regular", "NotoSansSymbols["],
        Weight::Regular,
    ),
    (&["NotoSansSymbols-Bold", "NotoSansSymbols["], Weight::Bold),
    (
        &[
            "NotoSansCJK-Regular",
            "NotoSansCJKjp-Regular",
            "NotoSansJP-Regular",
            "NotoSansJP[",
        ],
        Weight::Regular,
    ),
    (
        &[
            "NotoSansCJK-Bold",
            "NotoSansCJKjp-Bold",
            "NotoSansJP-Bold",
            "NotoSansJP[",
        ],
        Weight::Bold,
    ),
    (
        &["NotoSansCJKsc-Regular", "NotoSansSC-Regular", "NotoSansSC["],
        Weight::Regular,
    ),
    (
        &["NotoSansCJKsc-Bold", "NotoSansSC-Bold", "NotoSansSC["],
        Weight::Bold,
    ),
    (
        &["NotoSansCJKkr-Regular", "NotoSansKR-Regular", "NotoSansKR["],
        Weight::Regular,
    ),
    (
        &["NotoSansCJKkr-Bold", "NotoSansKR-Bold", "NotoSansKR["],
        Weight::Bold,
    ),
    (
        &["wqy-zenhei", "wqy-microhei", "DroidSansFallbackFull"],
        Weight::Regular,
    ),
    (
        &["wqy-zenhei", "wqy-microhei", "DroidSansFallbackFull"],
        Weight::Bold,
    ),
    (&["NotoSansThai-Regular", "NotoSansThai["], Weight::Regular),
    (&["NotoSansThai-Bold", "NotoSansThai["], Weight::Bold),
    (
        &["NotoSansDevanagari-Regular", "NotoSansDevanagari["],
        Weight::Regular,
    ),
    (
        &["NotoSansDevanagari-Bold", "NotoSansDevanagari["],
        Weight::Bold,
    ),
    (&["DejaVuSans-Bold"], Weight::Bold),
    (&["DejaVuSans."], Weight::Regular),
];

/// How deep the fonts' folders are looked into.
#[cfg(target_os = "linux")]
const MOST_DEEP: usize = 5;

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
        #[cfg(not(target_os = "linux"))]
        let faces = system_folder()
            .map(|folder| load(&folder))
            .unwrap_or_default();
        #[cfg(target_os = "linux")]
        let faces = linux::find();
        faces
    })
}

/// Where a Mac keeps its fonts.
#[cfg(target_os = "macos")]
fn system_folder() -> Option<PathBuf> {
    Some(PathBuf::from("/System/Library/Fonts"))
}

/// Where Windows keeps its fonts.
#[cfg(windows)]
fn system_folder() -> Option<PathBuf> {
    let windows = std::env::var_os("SystemRoot").or_else(|| std::env::var_os("windir"))?;
    Some(Path::new(&windows).join("Fonts"))
}

/// Maps every font of the list that `folder` has.
#[cfg(not(target_os = "linux"))]
pub fn load(folder: &Path) -> Vec<Face> {
    let files = FACES
        .iter()
        .map(|&(file, index, weight, drop)| (folder.join(file), index, weight, drop));
    map_all(files)
}

/// Maps those of these font files that are there.
fn map_all(files: impl Iterator<Item = (PathBuf, u32, Weight, f32)>) -> Vec<Face> {
    // A file standing behind two weights is mapped once.
    let mut mapped: Vec<(PathBuf, &'static [u8])> = Vec::new();
    let mut faces = Vec::new();
    for (path, index, weight, drop) in files {
        let known = mapped.iter().find(|(name, _)| *name == path);
        let bytes = match known {
            Some((_, bytes)) => *bytes,
            None => {
                let Some(bytes) = map::file(&path) else {
                    continue;
                };
                mapped.push((path.clone(), bytes));
                bytes
            }
        };
        if !has_face(bytes, index) {
            log::debug!("{} has no face {index}", path.display());
            continue;
        }
        faces.push(Face {
            name: format!("system-{}-{index}-{weight:?}", path.display()),
            bytes,
            index,
            weight,
            drop,
        });
    }
    faces
}

/// Whether this is a font, with a face of this number in it. Asked for a face
/// a file does not have, the drawing of text would stop the app.
fn has_face(bytes: &[u8], index: u32) -> bool {
    match bytes.get(..4) {
        Some(b"ttcf") => bytes
            .get(8..12)
            .and_then(|count| count.try_into().ok())
            .is_some_and(|count: [u8; 4]| index < u32::from_be_bytes(count)),
        Some([0, 1, 0, 0] | b"OTTO" | b"true") => index == 0,
        _ => false,
    }
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
    use std::fs::File;
    use std::path::Path;

    /// The file's bytes, mapped read-only and never unmapped: a font is in
    /// use until the app closes. `None` when it is not there, is empty, or
    /// cannot be mapped.
    pub fn file(path: &Path) -> Option<&'static [u8]> {
        let file = File::open(path).ok()?;
        if file.metadata().ok()?.len() == 0 {
            return None;
        }
        // SAFETY: the map is kept for the rest of the run. A font being
        // rewritten under it would be a system update, and is not guarded
        // against, as on Windows.
        let map = unsafe { memmap2::Mmap::map(&file) }.ok()?;
        let map: &'static memmap2::Mmap = Box::leak(Box::new(map));
        Some(&map[..])
    }
}

/// Finding a Linux desktop's fonts by their files' names.
#[cfg(target_os = "linux")]
mod linux {
    use std::path::{Path, PathBuf};

    use super::{FAMILIES, Face, MOST_DEEP, Weight};

    pub fn find() -> Vec<Face> {
        let mut roots = vec![
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
        ];
        if let Some(base) = directories::BaseDirs::new() {
            roots.push(base.data_dir().join("fonts"));
            roots.push(base.home_dir().join(".fonts"));
        }
        let mut files = Vec::new();
        for root in roots {
            walk(&root, 0, &mut files);
        }
        files.sort();
        super::map_all(chosen(&files).into_iter())
    }

    fn walk(folder: &Path, depth: usize, files: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(folder) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if depth < MOST_DEEP {
                    walk(&path, depth + 1, files);
                }
            } else {
                files.push(path);
            }
        }
    }

    /// For each row of [`FAMILIES`], the first of its names a file has.
    pub(super) fn chosen(files: &[PathBuf]) -> Vec<(PathBuf, u32, Weight, f32)> {
        let named = |prefix: &str| {
            files.iter().find(|file| {
                file.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(prefix))
            })
        };
        FAMILIES
            .iter()
            .filter_map(|(names, weight)| {
                let file = names.iter().find_map(|prefix| named(prefix))?;
                Some((file.clone(), 0, *weight, 0.0))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn a_face_a_file_does_not_have_is_not_asked_for() {
        let collection = [b"ttcf".as_slice(), &[0, 1, 0, 0], &[0, 0, 0, 2]].concat();
        assert!(has_face(&collection, 0));
        assert!(has_face(&collection, 1));
        assert!(!has_face(&collection, 2));
        assert!(has_face(&[0, 1, 0, 0, 0], 0));
        assert!(!has_face(&[0, 1, 0, 0, 0], 1));
        assert!(has_face(b"OTTO....", 0));
        assert!(!has_face(b"<html>", 0));
        assert!(!has_face(b"", 0));
    }

    fn weighing(file: PathBuf) -> impl Iterator<Item = (PathBuf, u32, Weight, f32)> {
        std::iter::once((file, 0, Weight::Regular, 0.0))
    }

    #[test]
    fn a_folder_with_no_fonts_gives_none_and_says_nothing() {
        let nowhere = std::env::temp_dir().join("spotified-no-fonts-here/a.ttf");
        assert!(map_all(weighing(nowhere)).is_empty());
    }

    #[test]
    fn a_file_that_is_empty_or_not_there_is_not_a_font() {
        let folder = Scratch::folder(&format!("spotified-fonts-{}", std::process::id()));
        std::fs::write(folder.join("empty.ttf"), b"").expect("an empty file");
        assert!(map_all(weighing(folder.join("empty.ttf"))).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn on_linux_a_font_is_found_wherever_its_distribution_put_it() {
        let files = [
            PathBuf::from("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"),
            PathBuf::from("/usr/share/fonts/google-noto-vf/NotoSansArabic[wght].ttf"),
        ];
        let found = linux::chosen(&files);
        let has = |file: &PathBuf, wanted: Weight| {
            found
                .iter()
                .any(|(path, _, weight, _)| path == file && *weight == wanted)
        };
        assert!(has(&files[0], Weight::Regular));
        assert!(has(&files[1], Weight::Regular));
        // A variable font stands behind both weights.
        assert!(has(&files[1], Weight::Bold));
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

//! Updates: look for a newer release, download its installer, check it,
//! and run it when asked.
//!
//! Releases are GitHub's, of this app's own repository. The installer is
//! not code-signed, so what vouches for a download is its SHA-256 in the
//! release's `SHA256SUMS.txt`: a file that does not match is thrown away.
//! Only an installed copy updates itself; a portable folder or a
//! development build is told so and left alone.
//!
//! On Windows the installer is Inno Setup's. A Mac's and a Linux copy have
//! no installer: the app's bundle, or its AppImage, is replaced where it
//! stands, which they allow of a running file.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::changelog::VERSION;

/// The newest release of this app.
const FEED: &str =
    "https://api.github.com/repos/DarkSavci/youtube-music-spotified-native/releases/latest";
/// How the asset this copy updates from is named: `…-setup.exe` and so on.
const INSTALLER_SUFFIX: &str = if cfg!(windows) {
    "-setup.exe"
} else if cfg!(target_os = "macos") {
    if cfg!(target_arch = "aarch64") {
        "-macos-arm64.tar.gz"
    } else {
        "-macos-x64.tar.gz"
    }
} else if cfg!(target_arch = "aarch64") {
    "-linux-aarch64.AppImage"
} else {
    "-linux-x86_64.AppImage"
};
const SUMS: &str = "SHA256SUMS.txt";
/// Beside the executable of an installed copy, and of nothing else.
#[cfg(windows)]
const UNINSTALLER: &str = "unins000.exe";
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Status {
    /// Not looked yet.
    #[default]
    Idle,
    /// This copy does not update itself.
    Unavailable,
    Checking,
    UpToDate,
    Downloading(String),
    /// Downloaded and checked: running the installer is all that is left.
    Ready {
        version: String,
        installer: PathBuf,
    },
    Failed(String),
}

impl Status {
    /// Whether a check would get in the way of what is already going on.
    pub fn busy(&self) -> bool {
        matches!(
            self,
            Status::Checking | Status::Downloading(_) | Status::Ready { .. } | Status::Unavailable
        )
    }
}

/// Whether this copy was put here by the installer.
#[cfg(windows)]
pub fn installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(UNINSTALLER).exists()))
        .unwrap_or(false)
}

/// Whether this copy is an AppImage, which can replace itself. One unpacked
/// from the `.tar.gz` is a portable folder.
#[cfg(target_os = "linux")]
pub fn installed() -> bool {
    std::env::var_os("APPIMAGE").is_some()
}

/// Whether this copy is a bundle somewhere it can be replaced. A Mac runs an
/// app not yet moved out of Downloads from a read-only copy elsewhere.
#[cfg(target_os = "macos")]
pub fn installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| bundle_of(&exe))
        .is_some_and(|bundle| !bundle.to_string_lossy().contains("/AppTranslocation/"))
}

/// The `.app` an executable is inside.
#[cfg(any(target_os = "macos", test))]
fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let bundle = exe.parent()?.parent()?.parent()?;
    let shape = exe.parent()?.ends_with("Contents/MacOS");
    (shape
        && bundle
            .extension()
            .is_some_and(|extension| extension == "app"))
    .then(|| bundle.to_path_buf())
}

/// Looks for a newer release and, if there is one, downloads its installer
/// into `folder`. `report` hears of each step; the last is also returned.
pub fn check(folder: &Path, report: impl Fn(Status)) -> Status {
    report(Status::Checking);
    let status = match look(folder, &report) {
        Ok(status) => status,
        Err(error) => Status::Failed(error),
    };
    report(status.clone());
    status
}

fn look(folder: &Path, report: &impl Fn(Status)) -> Result<Status, String> {
    let agent = agent();
    let Some(body) = fetch_text(&agent, FEED)? else {
        // No release has been published yet.
        return Ok(Status::UpToDate);
    };
    let Some(offer) = offer_in(&body, VERSION) else {
        return Ok(Status::UpToDate);
    };
    report(Status::Downloading(offer.version.clone()));
    let sums =
        fetch_text(&agent, &offer.sums_url)?.ok_or("the release has no list of checksums")?;
    let expected = expected_hash(&sums, &offer.installer_name)
        .ok_or("the release's checksums do not name its installer")?;
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let installer = folder.join(&offer.installer_name);
    // A download finished on an earlier run is not fetched again.
    if hash_of(&installer).ok().as_deref() != Some(expected.as_str()) {
        download(&agent, &offer.installer_url, &installer).map_err(|error| error.to_string())?;
        let actual = hash_of(&installer).map_err(|error| error.to_string())?;
        if actual != expected {
            let _ = std::fs::remove_file(&installer);
            return Err("the download did not match its checksum".into());
        }
    }
    Ok(Status::Ready {
        version: offer.version,
        installer,
    })
}

/// What the installer is asked to do when it is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    /// Start the app again: the update was asked for now.
    Relaunch,
    /// Nothing: the app was being closed anyway, and stays closed.
    StayClosed,
}

/// Runs the installer without its windows. The app must then quit, so its
/// files are free.
#[cfg(windows)]
pub fn install(installer: &Path, then: Then) -> io::Result<()> {
    std::process::Command::new(installer)
        .args(install_arguments(then))
        .spawn()
        .map(drop)
}

/// Replaces the AppImage this copy runs from. The app must then quit; the
/// new one is started once it has, as two would fight over the profile.
#[cfg(target_os = "linux")]
pub fn install(installer: &Path, then: Then) -> io::Result<()> {
    let image = PathBuf::from(
        std::env::var_os("APPIMAGE").ok_or_else(|| io::Error::other("not an AppImage"))?,
    );
    replace_image(installer, &image)?;
    if then == Then::Relaunch {
        start_after_this_ends(&image, &[])?;
    }
    Ok(())
}

/// Puts the download where the AppImage is, whole or not at all. The file in
/// use is left to whoever has it open, and the name goes to the new one.
#[cfg(target_os = "linux")]
fn replace_image(installer: &Path, image: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let beside = image.with_extension("new");
    std::fs::copy(installer, &beside)?;
    std::fs::set_permissions(&beside, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&beside, image)
}

/// Replaces the bundle this copy runs from with the one in the download. The
/// app must then quit; the new one is started once it has, as two would
/// fight over the profile.
#[cfg(target_os = "macos")]
pub fn install(installer: &Path, then: Then) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let bundle = bundle_of(&exe).ok_or_else(|| io::Error::other("not in a bundle"))?;
    let folder = bundle
        .parent()
        .ok_or_else(|| io::Error::other("a bundle with no folder"))?;
    let unpacked = folder.join(".spotified-update");
    let _ = std::fs::remove_dir_all(&unpacked);
    std::fs::create_dir_all(&unpacked)?;
    let status = std::process::Command::new("/usr/bin/tar")
        .arg("-xzf")
        .arg(installer)
        .arg("-C")
        .arg(&unpacked)
        .status()?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&unpacked);
        return Err(io::Error::other(format!("tar ended with {status}")));
    }
    let new = std::fs::read_dir(&unpacked)?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| io::Error::other("the download has no app in it"))?;
    let old = unpacked.join("old.app");
    std::fs::rename(&bundle, &old)?;
    if let Err(error) = std::fs::rename(&new, &bundle) {
        // Put back: an app that is gone is worse than one not updated.
        let _ = std::fs::rename(&old, &bundle);
        return Err(error);
    }
    let _ = std::fs::remove_dir_all(&unpacked);
    if then == Then::Relaunch {
        start_after_this_ends(Path::new("/usr/bin/open"), &[bundle.as_os_str()])?;
    }
    Ok(())
}

/// Starts `program` when this process has ended, from a shell that outlives it.
#[cfg(unix)]
fn start_after_this_ends(program: &Path, arguments: &[&std::ffi::OsStr]) -> io::Result<()> {
    start_after(std::process::id(), program, arguments)
}

/// Starts `program` when the process `pid` has ended.
#[cfg(unix)]
fn start_after(pid: u32, program: &Path, arguments: &[&std::ffi::OsStr]) -> io::Result<()> {
    const WAIT: &str = "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; shift; exec \"$@\"";
    std::process::Command::new("/bin/sh")
        .args(["-c", WAIT, "sh"])
        .arg(pid.to_string())
        .arg(program)
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(drop)
}

#[cfg(windows)]
fn install_arguments(then: Then) -> Vec<&'static str> {
    let mut arguments = vec!["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"];
    if then == Then::Relaunch {
        arguments.push("/RELAUNCH=1");
    }
    arguments
}

pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT * 20))
        .timeout_connect(Some(TIMEOUT))
        .http_status_as_error(false)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                // The system's TLS: the only one this build carries. Left
                // to its default, the agent panics on its first request.
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent()
}

/// The body at `url`; `None` if there is nothing there.
pub(crate) fn fetch_text(agent: &ureq::Agent, url: &str) -> Result<Option<String>, String> {
    let mut response = agent
        .get(url)
        .header(
            "User-Agent",
            concat!("spotified-native/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json, text/plain")
        .call()
        .map_err(|error| error.to_string())?;
    match response.status().as_u16() {
        200..=299 => response
            .body_mut()
            .read_to_string()
            .map(Some)
            .map_err(|error| error.to_string()),
        404 => Ok(None),
        status => Err(format!("the update server answered {status}")),
    }
}

pub(crate) fn download(agent: &ureq::Agent, url: &str, to: &Path) -> io::Result<()> {
    let response = agent
        .get(url)
        .header(
            "User-Agent",
            concat!("spotified-native/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(io::Error::other)?;
    if !response.status().is_success() {
        return Err(io::Error::other(format!(
            "the download answered {}",
            response.status()
        )));
    }
    // Written beside its place and renamed, so a half download is never
    // taken for a whole one.
    let partial = to.with_extension("part");
    let mut file = std::fs::File::create(&partial)?;
    io::copy(&mut response.into_body().into_reader(), &mut file)?;
    file.flush()?;
    drop(file);
    std::fs::rename(&partial, to)
}

pub(crate) fn hash_of(file: &Path) -> io::Result<String> {
    let mut file = std::fs::File::open(file)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// A release newer than this build, and where its files are.
#[derive(Debug, PartialEq, Eq)]
struct Offer {
    version: String,
    installer_name: String,
    installer_url: String,
    sums_url: String,
}

#[derive(Deserialize)]
struct ReleaseJson {
    tag_name: String,
    #[serde(default)]
    assets: Vec<AssetJson>,
}

#[derive(Deserialize)]
struct AssetJson {
    name: String,
    browser_download_url: String,
}

/// What GitHub's answer offers over `current`: nothing if it is no newer,
/// or lacks an installer or its checksums.
fn offer_in(body: &str, current: &str) -> Option<Offer> {
    let release: ReleaseJson = serde_json::from_str(body).ok()?;
    let version = release.tag_name.trim_start_matches('v');
    if !newer(version, current) {
        return None;
    }
    let asset = |wanted: &dyn Fn(&str) -> bool| {
        release
            .assets
            .iter()
            .find(|asset| wanted(&asset.name))
            .map(|asset| (asset.name.clone(), asset.browser_download_url.clone()))
    };
    let (installer_name, installer_url) = asset(&|name| name.ends_with(INSTALLER_SUFFIX))?;
    let (_, sums_url) = asset(&|name| name == SUMS)?;
    Some(Offer {
        version: version.to_owned(),
        installer_name,
        installer_url,
        sums_url,
    })
}

/// Whether `candidate` is a later version than `current`, part by part;
/// a pre-release (`0.5.0-beta.2`) comes before its release (`0.5.0`).
/// Anything that is not a number counts as zero.
fn newer(candidate: &str, current: &str) -> bool {
    let parts = |version: &str| -> Vec<u64> {
        let (release, pre) = version.split_once('-').unwrap_or((version, ""));
        let mut parts: Vec<u64> = release
            .split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect();
        parts.resize(parts.len().max(3), 0);
        // A release outranks every pre-release of it.
        parts.push(if pre.is_empty() {
            u64::MAX
        } else {
            pre.rsplit('.')
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0)
        });
        parts
    };
    parts(candidate) > parts(current)
}

/// The hash `sha256sum` wrote for `name`: `<hex>  <name>`, or `<hex> *<name>`.
pub(crate) fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| hash.to_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn an_appimage_is_replaced_whole_and_can_be_run() {
        use std::os::unix::fs::PermissionsExt;
        let dir = Scratch::folder(&format!("spotified-image-{}", std::process::id()));
        let (image, download) = (dir.join("app.AppImage"), dir.join("download"));
        std::fs::write(&image, b"old").expect("write");
        std::fs::write(&download, b"new").expect("write");
        replace_image(&download, &image).expect("replaced");
        assert_eq!(std::fs::read(&image).expect("read"), b"new");
        let mode = std::fs::metadata(&image)
            .expect("there")
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
        assert!(!dir.join("app.new").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_new_copy_starts_only_once_the_old_one_has_ended() {
        let dir = Scratch::folder(&format!("spotified-after-{}", std::process::id()));
        let started = dir.join("started");
        let script = dir.join("start.sh");
        std::fs::write(&script, format!("touch '{}'\n", started.display())).expect("write");
        let mut old = std::process::Command::new("sleep")
            .arg("1")
            .spawn()
            .expect("something to wait for");
        start_after(old.id(), Path::new("/bin/sh"), &[script.as_os_str()]).expect("waiting");
        std::thread::sleep(Duration::from_millis(300));
        assert!(!started.exists(), "started while the old one still ran");
        old.wait().expect("ended");
        let begun = std::time::Instant::now();
        while !started.exists() && begun.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(started.exists(), "never started");
    }

    #[test]
    fn a_bundle_is_found_from_the_executable_inside_it() {
        assert_eq!(
            bundle_of(Path::new("/Applications/A b.app/Contents/MacOS/app")),
            Some(PathBuf::from("/Applications/A b.app"))
        );
        assert_eq!(bundle_of(Path::new("/usr/local/bin/app")), None);
    }
    use crate::scratch::Scratch;

    const RELEASE: &str = r#"{
        "tag_name": "v0.3.0",
        "assets": [
            {"name": "youtube-music-spotified-native-0.3.0-windows-x64.zip", "browser_download_url": "https://example.com/zip"},
            {"name": "youtube-music-spotified-native-0.3.0-setup.exe", "browser_download_url": "https://example.com/setup"},
            {"name": "youtube-music-spotified-native-0.3.0-macos-arm64.tar.gz", "browser_download_url": "https://example.com/mac"},
            {"name": "youtube-music-spotified-native-0.3.0-macos-arm64.dmg", "browser_download_url": "https://example.com/dmg"},
            {"name": "youtube-music-spotified-native-0.3.0-macos-x64.tar.gz", "browser_download_url": "https://example.com/mac-intel"},
            {"name": "youtube-music-spotified-native-0.3.0-linux-x86_64.AppImage", "browser_download_url": "https://example.com/linux"},
            {"name": "youtube-music-spotified-native-0.3.0-linux-x86_64.tar.gz", "browser_download_url": "https://example.com/portable"},
            {"name": "youtube-music-spotified-native-0.3.0-linux-aarch64.AppImage", "browser_download_url": "https://example.com/linux-arm"},
            {"name": "SHA256SUMS.txt", "browser_download_url": "https://example.com/sums"}
        ]
    }"#;

    #[test]
    fn a_secure_address_that_cannot_be_reached_is_an_error_not_a_crash() {
        // Nothing listens there. What matters is that asking gets as far
        // as trying: an agent set up for TLS this build lacks panics.
        assert!(fetch_text(&agent(), "https://127.0.0.1:9/").is_err());
    }

    #[test]
    fn versions_are_compared_part_by_part_as_numbers() {
        assert!(newer("0.10.0", "0.9.9"));
        assert!(newer("1.0", "0.9.9"));
        assert!(newer("0.2.1", "0.2"));
        assert!(!newer("0.2.0", "0.2.0"));
        assert!(!newer("0.1.9", "0.2.0"));
        assert!(newer("0.5.0", "0.5.0-beta.2"));
        assert!(newer("0.5.0-beta.3", "0.5.0-beta.2"));
        assert!(!newer("0.5.0-beta.2", "0.5.0"));
        assert!(!newer("0.4.9", "0.5.0-beta.1"));
    }

    #[test]
    fn a_newer_release_is_offered_with_its_installer_and_checksums() {
        let offer = offer_in(RELEASE, "0.2.0").expect("an offer");
        assert_eq!(offer.version, "0.3.0");
        assert!(offer.installer_name.ends_with(INSTALLER_SUFFIX));
        let expected = if cfg!(windows) {
            "https://example.com/setup"
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "https://example.com/mac"
            } else {
                "https://example.com/mac-intel"
            }
        } else if cfg!(target_arch = "aarch64") {
            "https://example.com/linux-arm"
        } else {
            "https://example.com/linux"
        };
        assert_eq!(offer.installer_url, expected);
        assert_eq!(offer.sums_url, "https://example.com/sums");
    }

    #[test]
    fn a_release_that_is_no_newer_or_has_no_installer_is_not_offered() {
        assert_eq!(offer_in(RELEASE, "0.3.0"), None);
        let bare = r#"{"tag_name": "v9.0.0", "assets": []}"#;
        assert_eq!(offer_in(bare, "0.2.0"), None);
        assert_eq!(offer_in("not json", "0.2.0"), None);
    }

    #[test]
    fn the_checksum_is_found_by_the_files_name() {
        let sums = "AA11  other.zip\nbb22 *app-setup.exe\n";
        assert_eq!(
            expected_hash(sums, "app-setup.exe").as_deref(),
            Some("bb22")
        );
        assert_eq!(expected_hash(sums, "other.zip").as_deref(), Some("aa11"));
        assert_eq!(expected_hash(sums, "missing.exe"), None);
    }

    #[test]
    fn a_file_hashes_to_its_sha256() -> io::Result<()> {
        let path = Scratch::path(&format!("spotified-hash-{}", std::process::id()));
        std::fs::write(&path, b"abc")?;
        let hash = hash_of(&path)?;
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn an_update_asked_for_now_starts_the_app_again_and_one_at_quitting_does_not() {
        let now = install_arguments(Then::Relaunch);
        let quitting = install_arguments(Then::StayClosed);
        assert!(now.contains(&"/RELAUNCH=1"));
        assert!(!quitting.contains(&"/RELAUNCH=1"));
        // Neither shows a window or asks a question.
        for arguments in [now, quitting] {
            assert!(arguments.contains(&"/VERYSILENT"));
            assert!(arguments.contains(&"/SUPPRESSMSGBOXES"));
        }
    }
}

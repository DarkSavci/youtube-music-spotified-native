//! Updates: look for a newer release, download its installer, check it,
//! and run it when asked.
//!
//! Releases are GitHub's, of this app's own repository. The installer is
//! not code-signed, so what vouches for a download is its SHA-256 in the
//! release's `SHA256SUMS.txt`: a file that does not match is thrown away.
//! Only an installed copy updates itself; a portable folder or a
//! development build is told so and left alone.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::changelog::VERSION;

/// The newest release of this app.
const FEED: &str =
    "https://api.github.com/repos/DarkSavci/youtube-music-spotified-native/releases/latest";
/// How the installer's asset is named: `…-setup.exe`.
const INSTALLER_SUFFIX: &str = "-setup.exe";
const SUMS: &str = "SHA256SUMS.txt";
/// Beside the executable of an installed copy, and of nothing else.
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
pub fn installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(UNINSTALLER).exists()))
        .unwrap_or(false)
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

/// Runs the installer without its windows and asks it to start the app
/// again when it is done. The app must then quit, so its files are free.
pub fn install(installer: &Path) -> io::Result<()> {
    std::process::Command::new(installer)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/RELAUNCH=1",
        ])
        .spawn()
        .map(drop)
}

fn agent() -> ureq::Agent {
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
fn fetch_text(agent: &ureq::Agent, url: &str) -> Result<Option<String>, String> {
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

fn download(agent: &ureq::Agent, url: &str, to: &Path) -> io::Result<()> {
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

fn hash_of(file: &Path) -> io::Result<String> {
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

/// Whether `candidate` is a later version than `current`, part by part.
/// Anything that is not a number counts as zero.
fn newer(candidate: &str, current: &str) -> bool {
    let parts = |version: &str| -> Vec<u64> {
        version
            .split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    };
    let (candidate, current) = (parts(candidate), parts(current));
    let length = candidate.len().max(current.len());
    let at = |parts: &[u64], index: usize| parts.get(index).copied().unwrap_or(0);
    (0..length)
        .map(|index| at(&candidate, index).cmp(&at(&current, index)))
        .find(|order| order.is_ne())
        .is_some_and(|order| order.is_gt())
}

/// The hash `sha256sum` wrote for `name`: `<hex>  <name>`, or `<hex> *<name>`.
fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| hash.to_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASE: &str = r#"{
        "tag_name": "v0.3.0",
        "assets": [
            {"name": "youtube-music-spotified-native-0.3.0-windows-x64.zip", "browser_download_url": "https://example.com/zip"},
            {"name": "youtube-music-spotified-native-0.3.0-setup.exe", "browser_download_url": "https://example.com/setup"},
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
    }

    #[test]
    fn a_newer_release_is_offered_with_its_installer_and_checksums() {
        let offer = offer_in(RELEASE, "0.2.0").expect("an offer");
        assert_eq!(offer.version, "0.3.0");
        assert_eq!(offer.installer_url, "https://example.com/setup");
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
        let path = std::env::temp_dir().join(format!("spotified-hash-{}", std::process::id()));
        std::fs::write(&path, b"abc")?;
        let hash = hash_of(&path)?;
        std::fs::remove_file(&path)?;
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        Ok(())
    }
}

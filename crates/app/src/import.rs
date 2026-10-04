//! Signing in by copying the Electron app's session.
//!
//! Until this app has its own browser sign-in, an existing sign-in in
//! Youtube Music Spotified (the Electron app) can be copied across. The
//! Electron profile is only ever read: its `credentials.json` is parsed,
//! checked, and written to this app's own profile.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The Electron app's directory under the roaming application data.
const ELECTRON_DIR: &str = "Spotifier";
/// The cookie every signed-in Google session carries, in one of its forms.
const SESSION_COOKIES: [&str; 2] = ["SAPISID=", "__Secure-3PAPISID="];

#[derive(Debug)]
pub enum ImportError {
    /// The file was read but does not hold a signed-in session.
    NotSignedIn,
    Io(io::Error),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::NotSignedIn => write!(
                f,
                "Youtube Music Spotified is not signed in. Sign in there first."
            ),
            ImportError::Io(error) => write!(f, "The sign-in could not be copied: {error}."),
        }
    }
}

impl From<io::Error> for ImportError {
    fn from(error: io::Error) -> Self {
        ImportError::Io(error)
    }
}

/// The parts of the Electron app's `accounts.json` that say where the active
/// account keeps its credentials.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Accounts {
    active: Option<String>,
    accounts: Vec<Account>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Account {
    id: String,
    /// The first account, from before there were several, lives in the root.
    legacy: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Credentials {
    cookie: String,
}

/// Where the Electron app's active account keeps its credentials, if that
/// app has a profile with a credentials file in it.
pub fn electron_credentials() -> Option<PathBuf> {
    let root = directories::BaseDirs::new()?
        .config_dir()
        .join(ELECTRON_DIR);
    let path = credentials_in(&root);
    path.exists().then_some(path)
}

fn credentials_in(root: &Path) -> PathBuf {
    let accounts: Accounts = std::fs::read_to_string(root.join("accounts.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let active = accounts
        .accounts
        .iter()
        .find(|account| Some(&account.id) == accounts.active.as_ref());
    match active {
        Some(account) if !account.legacy => root
            .join("accounts")
            .join(&account.id)
            .join("credentials.json"),
        // The legacy account, or a profile from before accounts.json.
        _ => root.join("credentials.json"),
    }
}

/// Copies a signed-in session from `from` to `to`.
pub fn import(from: &Path, to: &Path) -> Result<(), ImportError> {
    let text = read_whole(from)?;
    let credentials: Credentials = serde_json::from_str(&text).unwrap_or_default();
    if !SESSION_COOKIES
        .iter()
        .any(|name| credentials.cookie.contains(name))
    {
        return Err(ImportError::NotSignedIn);
    }
    // Under another name first, so the core never reads half a file.
    let partial = to.with_extension("json.part");
    std::fs::write(&partial, text)?;
    std::fs::rename(&partial, to)?;
    Ok(())
}

/// Reads the file, once more if it did not parse: the Electron app rewrites
/// it when it refreshes the session, and a read can land mid-write.
fn read_whole(path: &Path) -> io::Result<String> {
    let text = std::fs::read_to_string(path)?;
    if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
        return Ok(text);
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    std::fs::read_to_string(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("spotified-import-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    #[test]
    fn a_signed_in_session_is_copied_whole() {
        let dir = scratch("copy");
        let from = dir.join("theirs.json");
        let to = dir.join("credentials.json");
        let session = r#"{"cookie":"SID=a; SAPISID=b","onBehalfOfUser":"123"}"#;
        std::fs::write(&from, session).expect("write");
        import(&from, &to).expect("import");
        assert_eq!(std::fs::read_to_string(&to).expect("read"), session);
    }

    #[test]
    fn a_file_without_a_session_is_refused_and_nothing_is_written() {
        let dir = scratch("refuse");
        let from = dir.join("theirs.json");
        let to = dir.join("credentials.json");
        std::fs::write(&from, r#"{"cookie":"CONSENT=yes"}"#).expect("write");
        assert!(matches!(import(&from, &to), Err(ImportError::NotSignedIn)));
        assert!(!to.exists());
    }

    #[test]
    fn the_legacy_account_keeps_its_credentials_in_the_root() {
        let root = scratch("legacy");
        let accounts = r#"{"active":"a","accounts":[{"id":"a","legacy":true}]}"#;
        std::fs::write(root.join("accounts.json"), accounts).expect("write");
        assert_eq!(credentials_in(&root), root.join("credentials.json"));
    }

    #[test]
    fn a_later_account_keeps_its_credentials_in_its_own_folder() {
        let root = scratch("later");
        let accounts =
            r#"{"active":"b","accounts":[{"id":"a","legacy":true},{"id":"b","legacy":false}]}"#;
        std::fs::write(root.join("accounts.json"), accounts).expect("write");
        assert_eq!(
            credentials_in(&root),
            root.join("accounts").join("b").join("credentials.json")
        );
    }

    #[test]
    fn a_profile_without_an_accounts_file_uses_the_root() {
        let root = scratch("bare");
        assert_eq!(credentials_in(&root), root.join("credentials.json"));
    }
}

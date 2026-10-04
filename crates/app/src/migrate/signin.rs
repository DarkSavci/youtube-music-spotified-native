//! Bringing a sign-in across: the Electron app's `credentials.json`, read,
//! checked for a session, and written to this app's own profile.

use std::fmt;
use std::io;
use std::path::Path;

use serde::Deserialize;

/// The cookie every signed-in Google session carries, in one of its forms.
const SESSION_COOKIES: [&str; 2] = ["SAPISID=", "__Secure-3PAPISID="];

#[derive(Debug)]
pub enum CopyError {
    /// The file was read but does not hold a signed-in session.
    NotSignedIn,
    Io(io::Error),
}

impl fmt::Display for CopyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CopyError::NotSignedIn => write!(f, "it is signed out in the old app"),
            CopyError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl From<io::Error> for CopyError {
    fn from(error: io::Error) -> Self {
        CopyError::Io(error)
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Credentials {
    cookie: String,
}

fn has_session(text: &str) -> bool {
    let credentials: Credentials = serde_json::from_str(text).unwrap_or_default();
    SESSION_COOKIES
        .iter()
        .any(|name| credentials.cookie.contains(name))
}

/// Whether the credentials file at `path` holds a signed-in session.
pub fn holds_session(path: &Path) -> bool {
    read_whole(path).is_ok_and(|text| has_session(&text))
}

/// Copies a signed-in session from `from` to `to`.
pub fn copy(from: &Path, to: &Path) -> Result<(), CopyError> {
    let text = read_whole(from)?;
    if !has_session(&text) {
        return Err(CopyError::NotSignedIn);
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
    use crate::migrate::leveldb::tests::scratch;

    #[test]
    fn a_signed_in_session_is_copied_whole() {
        let dir = scratch("signin-copy");
        let from = dir.join("theirs.json");
        let to = dir.join("credentials.json");
        let session = r#"{"cookie":"SID=a; SAPISID=b","onBehalfOfUser":"123"}"#;
        std::fs::write(&from, session).expect("write");
        assert!(holds_session(&from));
        copy(&from, &to).expect("copy");
        assert_eq!(std::fs::read_to_string(&to).expect("read"), session);
        // The original is as it was.
        assert_eq!(std::fs::read_to_string(&from).expect("read"), session);
    }

    #[test]
    fn a_file_without_a_session_is_refused_and_nothing_is_written() {
        let dir = scratch("signin-refuse");
        let from = dir.join("theirs.json");
        let to = dir.join("credentials.json");
        std::fs::write(&from, r#"{"cookie":"CONSENT=yes"}"#).expect("write");
        assert!(!holds_session(&from));
        assert!(matches!(copy(&from, &to), Err(CopyError::NotSignedIn)));
        assert!(!to.exists());
        assert!(!holds_session(&dir.join("absent.json")));
    }
}

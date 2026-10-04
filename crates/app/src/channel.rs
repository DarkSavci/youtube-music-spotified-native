//! Which of the account's channels is in use.
//!
//! One Google account can hold several YouTube channels, each with its own
//! library and likes. The core acts as whichever the credentials file
//! names, so choosing a channel is an edit to that file and a restart.

use std::io;
use std::path::Path;

use serde_json::Value;

/// The field of the credentials file that names the channel. Absent, or
/// empty, for the account's own.
const FIELD: &str = "onBehalfOfUser";

/// The channel the credentials name; empty for the account's own, and
/// when there are no credentials to read.
pub fn active(credentials: &Path) -> String {
    read(credentials)
        .ok()
        .and_then(|document| document.get(FIELD)?.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Names `channel_id` in the credentials, leaving the rest as it was.
pub fn select(credentials: &Path, channel_id: &str) -> io::Result<()> {
    let mut document = read(credentials)?;
    let fields = document
        .as_object_mut()
        .ok_or_else(|| io::Error::other("the credentials are not a JSON object"))?;
    if channel_id.is_empty() {
        fields.remove(FIELD);
    } else {
        fields.insert(FIELD.to_owned(), Value::from(channel_id));
    }
    // Written beside the file and renamed over it, so a crash mid-write
    // cannot leave half a sign-in.
    let partial = credentials.with_extension("json.part");
    std::fs::write(
        &partial,
        serde_json::to_vec(&document).map_err(io::Error::other)?,
    )?;
    std::fs::rename(&partial, credentials)
}

fn read(credentials: &Path) -> io::Result<Value> {
    serde_json::from_slice(&std::fs::read(credentials)?).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str, contents: &str) -> io::Result<std::path::PathBuf> {
        let dir = std::env::temp_dir().join(format!("spotified-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("credentials.json");
        std::fs::write(&path, contents)?;
        Ok(path)
    }

    #[test]
    fn choosing_a_channel_keeps_the_rest_of_the_sign_in() -> io::Result<()> {
        let path = scratch(
            "channel",
            r#"{"cookie":"c","extra":{"x-goog-authuser":"0"}}"#,
        )?;
        assert_eq!(active(&path), "");
        select(&path, "123")?;
        assert_eq!(active(&path), "123");
        let document = read(&path)?;
        assert_eq!(document["cookie"], "c");
        assert_eq!(document["extra"]["x-goog-authuser"], "0");
        Ok(())
    }

    #[test]
    fn going_back_to_the_accounts_own_channel_removes_the_name() -> io::Result<()> {
        let path = scratch("own-channel", r#"{"cookie":"c","onBehalfOfUser":"123"}"#)?;
        select(&path, "")?;
        assert_eq!(active(&path), "");
        assert!(read(&path)?.get(FIELD).is_none());
        Ok(())
    }

    #[test]
    fn without_credentials_there_is_no_channel_and_none_can_be_chosen() {
        let missing = std::env::temp_dir().join("spotified-no-such-dir/credentials.json");
        assert_eq!(active(&missing), "");
        assert!(select(&missing, "123").is_err());
    }
}

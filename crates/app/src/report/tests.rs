use super::*;
use crate::settings::Settings;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("spotified-report-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn request(root: &Path) -> Request {
    let logs = root.join("logs");
    std::fs::create_dir_all(&logs).expect("a logs folder");
    Request {
        logs,
        into: root.join("downloads"),
        core: None,
        resolver: None,
        credentials: root.join("credentials.json"),
        audio_cache: root.join("audio-cache"),
        page: json!({ "page": "Home" }),
    }
}

#[test]
fn a_time_is_stamped_for_a_file_name_and_for_reading() {
    let at = OffsetDateTime::from_unix_timestamp(1_791_127_812).expect("a time");
    assert_eq!(stamp(at), "20261004-153012");
    assert_eq!(clock(at), "2026-10-04 15:30:12");
}

#[test]
fn only_the_apps_logs_are_listed_and_the_newest_comes_first() {
    let root = scratch("list");
    for name in ["app-1.log", "app-2.log", "notes.txt", "other.log"] {
        std::fs::write(root.join(name), name).expect("write");
    }
    let names: Vec<String> = logs_newest_first(&root)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    // Written in the same instant, they fall back to their names, which
    // carry the time they were started.
    assert_eq!(names.len(), 2);
    assert!(names.iter().all(|name| name.starts_with("app-")));
    assert!(logs_newest_first(&root.join("nowhere")).is_empty());
}

#[test]
fn the_summary_says_what_exists_without_saying_what_it_holds() {
    let root = scratch("summary");
    let request = request(&root);
    std::fs::write(&request.credentials, r#"{"cookie":"SAPISID=secret"}"#).expect("write");
    let said = summary(&request);
    assert!(said.starts_with("Youtube Music Spotified diagnostics\n"));
    assert!(said.contains("credentials: present"));
    assert!(said.contains("yt cookies:  absent"));
    assert!(said.contains("core:        not running"));
    assert!(said.contains("yt-dlp:      not found"));
    assert!(said.contains("song cache:  none"));
    assert!(!said.contains("secret"));
}

#[test]
fn a_folders_size_counts_its_files() {
    let root = scratch("size");
    std::fs::write(root.join("one"), vec![0u8; 1 << 20]).expect("write");
    std::fs::write(root.join("two"), vec![0u8; 1 << 20]).expect("write");
    assert_eq!(folder_size(&root), "2 files, 2 MB");
}

#[test]
fn the_page_state_leaves_out_what_is_the_persons_own_business() {
    let mut settings = Settings {
        device_id: "native-0123".into(),
        recent_searches: vec!["a private search".into()],
        together_name: "Ada".into(),
        ..Settings::default()
    };
    settings.crossfade_seconds = 4;
    let state = State::new(settings);
    let page = page_state(&state);
    let text = page.to_string();
    assert!(!text.contains("native-0123"));
    assert!(!text.contains("a private search"));
    assert!(!text.contains("together_name"));
    assert_eq!(page["settings"]["crossfade_seconds"], 4);
    assert_eq!(page["signedIn"], false);
    assert_eq!(page["player"], Value::Null);
}

/// The whole thing, with the `tar` Windows ships: a zip Explorer can open,
/// holding the summary and the logs with their secrets taken out.
#[cfg(windows)]
#[test]
fn a_report_is_a_zip_of_the_summary_and_the_scrubbed_logs() {
    let root = scratch("zip");
    let request = request(&root);
    let secret = "level=INFO msg=request Cookie: SAPISID=abc123; SID=def456\n\
                  playing dQw4w9WgXcQ for ada@example.com\n";
    std::fs::write(request.logs.join("app-100.log"), secret).expect("write");
    std::fs::write(request.logs.join("app-200.log"), "second launch\n").expect("write");

    let zip = save(&request).expect("a report");
    assert_eq!(zip.parent(), Some(request.into.as_path()));
    let name = zip
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a name");
    assert!(name.starts_with("ytms-diagnostics-") && name.ends_with(".zip"));

    // Listed by plain names: Explorer shows a zip of "./" entries as empty.
    let listing = crate::resolver::system_tar()
        .arg("-tf")
        .arg(&zip)
        .stdout(Stdio::piped())
        .output()
        .expect("tar");
    let listing = String::from_utf8_lossy(&listing.stdout);
    let mut entries: Vec<&str> = listing.lines().map(str::trim).collect();
    entries.sort_unstable();
    assert_eq!(entries, ["app-100.log", "app-200.log", "info.txt"]);

    let unpacked = root.join("unpacked");
    std::fs::create_dir_all(&unpacked).expect("a folder");
    let status = crate::resolver::system_tar()
        .arg("-xf")
        .arg(&zip)
        .arg("-C")
        .arg(&unpacked)
        .status()
        .expect("tar");
    assert!(status.success());
    let log = std::fs::read_to_string(unpacked.join("app-100.log")).expect("read");
    assert!(log.contains("Cookie: <redacted>"));
    assert!(log.contains("playing dQw4w9WgXcQ for <email>"));
    assert!(!log.contains("abc123") && !log.contains("ada@example.com"));
    let info = std::fs::read_to_string(unpacked.join("info.txt")).expect("read");
    assert!(info.contains("page state:"));
}

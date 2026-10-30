//! Signing in through the person's own browser.
//!
//! Google refuses to sign in to anything that looks automated, including an
//! installed Chrome started with a debugging connection. So the sign-in
//! happens in two steps, as in the Electron app:
//!
//! 1. An ordinary browser window is opened on a throwaway profile, with
//!    nothing attached. On Windows the only thing watched is its window
//!    title, the same thing the taskbar shows; when it reads "YouTube Music"
//!    the sign-in has landed and the window is closed, which makes the
//!    browser write its cookies to disk. Elsewhere there is no such easy
//!    way to read a title, so what is watched is the profile's cookie file,
//!    for the name of the cookie a sign-in leaves. Closing it by hand works
//!    too.
//! 2. The profile is reopened without a window, never loading a Google
//!    page, only to read those cookies back. They become this app's
//!    credentials and the profile is deleted.
//!
//! None of the person's own browsing profile is read or changed.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;

const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fmusic.youtube.com%2F";
/// How long the reopened browser has to say where it is listening.
const BROWSER_START: Duration = Duration::from_secs(15);
const REPLY_TIMEOUT: Duration = Duration::from_secs(20);
/// Told to the browser in both steps, so that a profile thrown away after
/// one sign-in asks for nothing of the system's password store.
#[cfg(target_os = "macos")]
const STORE_FLAGS: &[&str] = &["--use-mock-keychain"];
#[cfg(target_os = "linux")]
const STORE_FLAGS: &[&str] = &["--password-store=basic"];
#[cfg(windows)]
const STORE_FLAGS: &[&str] = &[];
/// Where a profile keeps its cookies, which depends on the version. Their
/// names are not encrypted.
#[cfg(unix)]
const COOKIE_FILES: [&str; 2] = ["Default/Cookies", "Default/Network/Cookies"];
/// How often the cookie files are looked at. Chrome writes them every half
/// minute or so.
#[cfg(unix)]
const LOOK_EVERY: Duration = Duration::from_secs(1);
/// How long a browser has to save and leave once told to.
#[cfg(unix)]
const BROWSER_STOP: Duration = Duration::from_secs(10);
/// How long a sign-in may take. On a Mac closing a browser's last window does
/// not end it, so a sign-in given up on is not otherwise known of.
#[cfg(unix)]
const SIGN_IN_MOST: Duration = Duration::from_secs(15 * 60);

/// Cookies the core needs. Anything else in the profile is left behind.
const WANTED: [&str; 23] = [
    "SAPISID",
    "__Secure-1PAPISID",
    "__Secure-3PAPISID",
    "SID",
    "__Secure-1PSID",
    "__Secure-3PSID",
    "HSID",
    "SSID",
    "APISID",
    "LOGIN_INFO",
    "__Secure-1PSIDTS",
    "__Secure-3PSIDTS",
    "SIDCC",
    "__Secure-1PSIDCC",
    "__Secure-3PSIDCC",
    "VISITOR_INFO1_LIVE",
    "VISITOR_PRIVACY_METADATA",
    "PREF",
    "YSC",
    "__Secure-YNID",
    "__Secure-ROLLOUT_TOKEN",
    "__Secure-BUCKET",
    "wide",
];
/// A signed-in session has LOGIN_INFO and one of these: the first marks an
/// account, and request signatures are computed over the second.
const SAPISID: [&str; 3] = ["SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID"];
/// Where Google keeps them, in the order a duplicate is settled: YouTube's
/// own copy wins.
const DOMAINS: [&str; 3] = [".youtube.com", "music.youtube.com", ".google.com"];

#[derive(Debug)]
pub enum SignInError {
    /// Neither Chrome nor Edge is installed.
    NoBrowser,
    /// The window was closed before a sign-in finished.
    NotSignedIn,
    Failed(String),
}

impl fmt::Display for SignInError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignInError::NoBrowser if cfg!(windows) => {
                write!(f, "Signing in needs Chrome or Edge installed.")
            }
            SignInError::NoBrowser => {
                write!(
                    f,
                    "Signing in needs Chrome, Edge, Chromium or Brave installed."
                )
            }
            SignInError::NotSignedIn => {
                write!(f, "The browser was closed before the sign-in finished.")
            }
            SignInError::Failed(reason) => write!(f, "The sign-in could not be read: {reason}."),
        }
    }
}

impl From<io::Error> for SignInError {
    fn from(error: io::Error) -> Self {
        SignInError::Failed(error.to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Cookie {
    name: String,
    value: String,
    domain: String,
}

/// Chrome, or failing that another of its kind, where a Mac's are put.
#[cfg(target_os = "macos")]
pub fn browser() -> Option<PathBuf> {
    const APPS: [&str; 4] = [
        "Google Chrome.app/Contents/MacOS/Google Chrome",
        "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "Chromium.app/Contents/MacOS/Chromium",
        "Brave Browser.app/Contents/MacOS/Brave Browser",
    ];
    let home = std::env::var_os("HOME").map(|home| Path::new(&home).join("Applications"));
    let folders: Vec<PathBuf> = std::iter::once(PathBuf::from("/Applications"))
        .chain(home)
        .collect();
    APPS.iter()
        .flat_map(|app| folders.iter().map(move |folder| folder.join(app)))
        .find(|path| path.exists())
}

/// Chrome, or failing that another of its kind, on the `PATH`.
#[cfg(target_os = "linux")]
pub fn browser() -> Option<PathBuf> {
    const NAMES: [&str; 7] = [
        "google-chrome-stable",
        "google-chrome",
        "chromium",
        "chromium-browser",
        "microsoft-edge-stable",
        "microsoft-edge",
        "brave-browser",
    ];
    let path = std::env::var_os("PATH")?;
    let folders: Vec<PathBuf> = std::env::split_paths(&path).collect();
    NAMES
        .iter()
        .flat_map(|name| folders.iter().map(move |folder| folder.join(name)))
        .find(|path| path.is_file())
}

/// Chrome, or failing that Edge, where their installers put them.
#[cfg(windows)]
pub fn browser() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let chrome = "Google/Chrome/Application/chrome.exe";
    let edge = "Microsoft/Edge/Application/msedge.exe";
    [
        var("ProgramFiles").map(|dir| dir.join(chrome)),
        var("ProgramFiles(x86)").map(|dir| dir.join(chrome)),
        var("LOCALAPPDATA").map(|dir| dir.join(chrome)),
        var("ProgramFiles(x86)").map(|dir| dir.join(edge)),
        var("ProgramFiles").map(|dir| dir.join(edge)),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.exists())
}

/// Runs the whole sign-in and writes `credentials`. Blocks until the
/// browser window is closed, so it is called from a thread of its own.
/// `scratch` is where the throwaway profile lives while it does.
pub fn sign_in(scratch: &Path, credentials: &Path) -> Result<(), SignInError> {
    let browser = browser().ok_or(SignInError::NoBrowser)?;
    let profile = scratch.join("signin-browser");
    let _ = std::fs::remove_dir_all(&profile);
    let result = show_sign_in(&browser, &profile)
        .and_then(|()| read_cookies(&browser, &profile))
        .and_then(|cookies| cookie_header(&cookies).ok_or(SignInError::NotSignedIn));
    // The profile holds a live session; it goes whatever happened.
    let _ = std::fs::remove_dir_all(&profile);
    write_credentials(credentials, &result?)?;
    Ok(())
}

/// The sign-in page in an ordinary window, started.
fn open_sign_in(browser: &Path, profile: &Path) -> Result<Child, SignInError> {
    Ok(Command::new(browser)
        .arg(format!("--user-data-dir={}", profile.display()))
        .args(STORE_FLAGS)
        .args([
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-sync",
            "--new-window",
            SIGN_IN_URL,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?)
}

/// Step one: the sign-in page, until the browser has been closed, or told to
/// close because the sign-in has left its cookie.
#[cfg(unix)]
fn show_sign_in(browser: &Path, profile: &Path) -> Result<(), SignInError> {
    let mut window = open_sign_in(browser, profile)?;
    let deadline = Instant::now() + SIGN_IN_MOST;
    loop {
        if window.try_wait()?.is_some() {
            return Ok(());
        }
        let signed_in = COOKIE_FILES
            .iter()
            .any(|file| has_sign_in_cookie(&profile.join(file)));
        if signed_in || Instant::now() >= deadline {
            stop_browser(&mut window);
            return Ok(());
        }
        std::thread::sleep(LOOK_EVERY);
    }
}

/// Whether the cookie file names the cookie YouTube leaves on a sign-in.
#[cfg(unix)]
fn has_sign_in_cookie(file: &Path) -> bool {
    std::fs::read(file).is_ok_and(|bytes| {
        bytes
            .windows(b"LOGIN_INFO".len())
            .any(|window| window == b"LOGIN_INFO")
    })
}

/// Asks the browser to leave, which it takes as a reason to save its cookies,
/// and makes it if it will not.
#[cfg(unix)]
fn stop_browser(browser: &mut Child) {
    let told = Command::new("kill")
        .args(["-TERM", &browser.id().to_string()])
        .status();
    if let Err(error) = told {
        log::debug!("the browser could not be told to leave: {error}");
    }
    let deadline = Instant::now() + BROWSER_STOP;
    while Instant::now() < deadline {
        if !matches!(browser.try_wait(), Ok(None)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = browser.kill();
    let _ = browser.wait();
}

/// Step one: the sign-in page in an ordinary window, until it is closed.
#[cfg(windows)]
fn show_sign_in(browser: &Path, profile: &Path) -> Result<(), SignInError> {
    let mut window = open_sign_in(browser, profile)?;
    let mut watcher = close_when_signed_in(window.id());
    let exited = window.wait();
    if let Some(watcher) = &mut watcher {
        let _ = watcher.kill();
        let _ = watcher.wait();
    }
    exited?;
    Ok(())
}

/// Closes the browser window, as clicking its X would, once its title says
/// YouTube Music has loaded. Without this the person closes it themselves.
#[cfg(windows)]
fn close_when_signed_in(browser_pid: u32) -> Option<Child> {
    // The title is read the way the taskbar reads it; nothing touches the
    // page. Two seconds' grace lets the browser finish writing cookies.
    let script = format!(
        "$p = Get-Process -Id {browser_pid} -ErrorAction SilentlyContinue; \
         while ($p -and -not $p.HasExited) {{ \
           $p.Refresh(); \
           if ($p.MainWindowTitle -like '*YouTube Music*') {{ \
             Start-Sleep 2; $p.CloseMainWindow() | Out-Null; break \
           }}; \
           Start-Sleep 1 \
         }}"
    );
    let mut command = Command::new("powershell");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::sidecar::hide_console(&mut command);
    command
        .spawn()
        .inspect_err(|error| log::debug!("no title watcher: {error}"))
        .ok()
}

/// Step two: the profile reopened without a window, asked for its cookies
/// over the DevTools protocol.
fn read_cookies(browser: &Path, profile: &Path) -> Result<Vec<Cookie>, SignInError> {
    let port_file = profile.join("DevToolsActivePort");
    let _ = std::fs::remove_file(&port_file);
    let mut headless = Command::new(browser)
        .arg(format!("--user-data-dir={}", profile.display()))
        .args(STORE_FLAGS)
        .args([
            "--headless=new",
            "--remote-debugging-port=0",
            "--no-first-run",
            "--no-default-browser-check",
            "about:blank",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let cookies = devtools_address(&port_file).and_then(|address| ask_for_cookies(&address));
    let _ = headless.kill();
    let _ = headless.wait();
    cookies
}

/// Where the browser's DevTools endpoint is: it writes the port and the
/// path, a line each, into its profile once it is listening.
fn devtools_address(port_file: &Path) -> Result<String, SignInError> {
    let deadline = Instant::now() + BROWSER_START;
    loop {
        if let Ok(text) = std::fs::read_to_string(port_file)
            && let [port, path] = text.lines().collect::<Vec<_>>().as_slice()
        {
            return Ok(format!("ws://127.0.0.1:{port}{path}"));
        }
        if Instant::now() >= deadline {
            return Err(SignInError::Failed("the browser did not start".into()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn ask_for_cookies(address: &str) -> Result<Vec<Cookie>, SignInError> {
    #[derive(Deserialize)]
    struct Reply {
        id: Option<u64>,
        result: Option<Cookies>,
    }
    #[derive(Deserialize)]
    struct Cookies {
        cookies: Vec<Cookie>,
    }
    let failed = |error: tungstenite::Error| SignInError::Failed(error.to_string());

    let (mut socket, _) = tungstenite::connect(address).map_err(failed)?;
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_ref() {
        stream.set_read_timeout(Some(REPLY_TIMEOUT))?;
    }
    let request = json!({ "id": 1, "method": "Storage.getCookies" }).to_string();
    socket.send(request.into()).map_err(failed)?;
    // The browser may say other things first; the answer carries our id.
    loop {
        let message = socket.read().map_err(failed)?;
        let Ok(text) = message.to_text() else {
            continue;
        };
        if let Ok(Reply {
            id: Some(1),
            result,
        }) = serde_json::from_str(text)
        {
            return result
                .map(|result| result.cookies)
                .ok_or_else(|| SignInError::Failed("no cookies in the reply".into()));
        }
    }
}

/// The `Cookie` header for a signed-in session, or `None` if these cookies
/// are not one. Each wanted cookie once, YouTube's copy before Google's.
fn cookie_header(cookies: &[Cookie]) -> Option<String> {
    let chosen: Vec<&Cookie> = WANTED
        .iter()
        .filter_map(|name| {
            DOMAINS.iter().find_map(|domain| {
                cookies
                    .iter()
                    .find(|cookie| cookie.name == *name && cookie.domain == *domain)
            })
        })
        .collect();
    let has = |name: &str| chosen.iter().any(|cookie| cookie.name == name);
    if !has("LOGIN_INFO") || !SAPISID.iter().any(|name| has(name)) {
        return None;
    }
    Some(
        chosen
            .iter()
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// Written whole and then moved into place: half a file would look valid
/// and fail only at the first request.
fn write_credentials(path: &Path, cookie_header: &str) -> io::Result<()> {
    let json = serde_json::to_string_pretty(&json!({ "cookie": cookie_header }))?;
    let partial = path.with_extension("json.part");
    std::fs::write(&partial, json)?;
    std::fs::rename(&partial, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn the_cookie_a_sign_in_leaves_is_found_by_its_name() {
        let dir =
            crate::scratch::Scratch::folder(&format!("spotified-signin-{}", std::process::id()));
        let file = dir.join("Cookies");
        assert!(!has_sign_in_cookie(&file), "no file, no sign-in");
        std::fs::write(&file, b"SQLite format 3 ...SID...").expect("write");
        assert!(!has_sign_in_cookie(&file));
        std::fs::write(&file, b"SQLite format 3 ...LOGIN_INFO...").expect("write");
        assert!(has_sign_in_cookie(&file));
    }
    use crate::scratch::Scratch;

    fn cookie(name: &str, value: &str, domain: &str) -> Cookie {
        Cookie {
            name: name.into(),
            value: value.into(),
            domain: domain.into(),
        }
    }

    #[test]
    fn a_signed_in_session_becomes_a_header_of_the_wanted_cookies() {
        let cookies = [
            cookie("LOGIN_INFO", "li", ".youtube.com"),
            cookie("SAPISID", "sap", ".youtube.com"),
            cookie("_ga", "tracking", ".youtube.com"),
            cookie("SID", "sid", ".google.com"),
        ];
        assert_eq!(
            cookie_header(&cookies).as_deref(),
            Some("SAPISID=sap; SID=sid; LOGIN_INFO=li")
        );
    }

    #[test]
    fn youtubes_copy_of_a_cookie_wins_over_googles() {
        let cookies = [
            cookie("LOGIN_INFO", "li", ".youtube.com"),
            cookie("SAPISID", "from-google", ".google.com"),
            cookie("SAPISID", "from-youtube", ".youtube.com"),
        ];
        let header = cookie_header(&cookies).expect("a header");
        assert!(header.contains("SAPISID=from-youtube"));
        assert!(!header.contains("from-google"));
    }

    #[test]
    fn a_session_without_an_account_is_not_signed_in() {
        let anonymous = [cookie("VISITOR_INFO1_LIVE", "v", ".youtube.com")];
        assert_eq!(cookie_header(&anonymous), None);
        let half = [cookie("LOGIN_INFO", "li", ".youtube.com")];
        assert_eq!(cookie_header(&half), None);
    }

    /// Opens a real browser without a window on an empty profile and asks
    /// it for its cookies: the plumbing of step two, short of a sign-in.
    #[test]
    #[ignore = "starts the installed browser; run with --ignored"]
    fn a_fresh_profile_can_be_asked_for_its_cookies() {
        let browser = browser().expect("Chrome or Edge");
        let profile = Scratch::path("spotified-signin-test");
        let cookies = read_cookies(&browser, &profile).expect("a reply");
        assert_eq!(cookie_header(&cookies), None);
    }
}

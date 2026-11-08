//! Signing in through the person's own browser.
//!
//! Google refuses to sign in to anything that looks automated, including an
//! installed Chrome started with a debugging connection. So the sign-in
//! happens in two steps, as in the Electron app:
//!
//! 1. An ordinary browser window is opened on a throwaway profile, with
//!    nothing attached. On Windows the browser's own windows are watched
//!    for their title, the same thing the taskbar shows; when one reads
//!    "YouTube Music" the sign-in has landed and the window is closed,
//!    which makes the browser write its cookies to disk. Elsewhere there is
//!    no such easy way to read a title, so what is watched is the profile's
//!    cookie file, for the name of the cookie a sign-in leaves. Closing it
//!    by hand works too.
//! 2. The profile is reopened without a window, never loading a Google
//!    page, only to read those cookies back. They become this app's
//!    credentials and the profile is deleted.
//!
//! None of the person's own browsing profile is read or changed. The whole
//! thing can be called off at any point through a [`Control`].

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
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
/// How often the window is looked at: often enough that Cancel and "show
/// the window" feel immediate.
const LOOK_EVERY: Duration = Duration::from_millis(250);
/// How often the cookie files are read. Chrome writes them every half
/// minute or so.
#[cfg(unix)]
const READ_COOKIES_EVERY: Duration = Duration::from_secs(1);
/// How long a browser has to save and leave once told to.
const BROWSER_STOP: Duration = Duration::from_secs(10);
/// How long a browser has to let go of its profile once it has left.
const PROFILE_RELEASE: Duration = Duration::from_secs(10);
/// How long a sign-in may take before it is given up on. On a Mac closing a
/// browser's last window does not end it, so a sign-in abandoned there is
/// not otherwise known of.
const SIGN_IN_MOST: Duration = Duration::from_secs(15 * 60);
/// The throwaway profiles are named for this, with something after it.
const PROFILE_PREFIX: &str = "signin-browser";

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
    /// No browser of Chrome's kind is installed.
    NoBrowser,
    /// The window was closed before a sign-in finished.
    NotSignedIn,
    /// The window was left open longer than [`SIGN_IN_MOST`].
    TimedOut,
    /// Called off through the [`Control`].
    Cancelled,
    Failed(String),
}

impl fmt::Display for SignInError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignInError::NoBrowser if cfg!(windows) => write!(
                f,
                "Signing in needs Google Chrome, Microsoft Edge or Brave, and none of them \
                 was found."
            ),
            SignInError::NoBrowser => write!(
                f,
                "Signing in needs Google Chrome, Chromium, Microsoft Edge or Brave, and none \
                 of them was found."
            ),
            SignInError::NotSignedIn => write!(
                f,
                "The browser window closed before you finished signing in. Try again, and \
                 leave it open until YouTube Music has loaded."
            ),
            SignInError::TimedOut => write!(
                f,
                "The sign-in was open for {} minutes without finishing, so it was stopped.",
                SIGN_IN_MOST.as_secs() / 60
            ),
            SignInError::Cancelled => write!(f, "The sign-in was cancelled."),
            SignInError::Failed(reason) => {
                write!(f, "Something went wrong while signing in ({reason}).")
            }
        }
    }
}

impl From<io::Error> for SignInError {
    fn from(error: io::Error) -> Self {
        SignInError::Failed(error.to_string())
    }
}

/// What the window can ask of a sign-in under way, from another thread.
#[derive(Debug, Default)]
pub struct Control {
    cancel: AtomicBool,
    raise: AtomicBool,
}

impl Control {
    /// Stop: the browser is closed and nothing is saved.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Bring the browser's window to the front, where the system allows it.
    pub fn raise(&self) {
        self.raise.store(true, Ordering::Relaxed);
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    #[cfg(windows)]
    fn take_raise(&self) -> bool {
        self.raise.swap(false, Ordering::Relaxed)
    }

    fn check(&self) -> Result<(), SignInError> {
        if self.cancelled() {
            Err(SignInError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Whether the browser's window can be brought forward from here.
pub const CAN_RAISE: bool = cfg!(windows);

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

/// Chrome, or failing that Edge or Brave, where their installers put them.
#[cfg(windows)]
pub fn browser() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let chrome = "Google/Chrome/Application/chrome.exe";
    let edge = "Microsoft/Edge/Application/msedge.exe";
    let brave = "BraveSoftware/Brave-Browser/Application/brave.exe";
    [
        var("ProgramFiles").map(|dir| dir.join(chrome)),
        var("ProgramFiles(x86)").map(|dir| dir.join(chrome)),
        var("LOCALAPPDATA").map(|dir| dir.join(chrome)),
        var("ProgramFiles(x86)").map(|dir| dir.join(edge)),
        var("ProgramFiles").map(|dir| dir.join(edge)),
        var("LOCALAPPDATA").map(|dir| dir.join(edge)),
        var("ProgramFiles").map(|dir| dir.join(brave)),
        var("LOCALAPPDATA").map(|dir| dir.join(brave)),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.exists())
}

/// What the browser a sign-in would open is called, to name it to the
/// person; `None` if there is none.
pub fn browser_name() -> Option<&'static str> {
    let path = browser()?;
    let file = path.file_name()?.to_string_lossy().to_lowercase();
    Some(if file.contains("edge") {
        "Microsoft Edge"
    } else if file.contains("brave") {
        "Brave"
    } else if file.contains("chromium") {
        "Chromium"
    } else {
        "Google Chrome"
    })
}

/// Runs the whole sign-in and writes `credentials`. Blocks until the
/// browser window is closed or the sign-in is called off, so it is called
/// from a thread of its own. `scratch` is where the throwaway profile lives
/// while it does; `reading` is called once the window has gone and the
/// sign-in is being read back.
pub fn sign_in(
    scratch: &Path,
    credentials: &Path,
    control: &Control,
    reading: impl FnOnce(),
) -> Result<(), SignInError> {
    let browser = browser().ok_or(SignInError::NoBrowser)?;
    let base = profile_base(scratch, &browser);
    forget_profiles(&base);
    let profile = base.join(format!("{PROFILE_PREFIX}-{}", std::process::id()));
    let result = show_sign_in(&browser, &profile, control, SIGN_IN_URL)
        .and_then(|()| {
            control.check()?;
            reading();
            wait_for_release(&profile, control)?;
            read_cookies(&browser, &profile)
        })
        .and_then(|cookies| cookie_header(&cookies).ok_or(SignInError::NotSignedIn))
        .and_then(|header| control.check().map(|()| header));
    // The profile holds a live session; it goes whatever happened.
    let _ = std::fs::remove_dir_all(&profile);
    write_credentials(credentials, &result?)?;
    Ok(())
}

/// Where the throwaway profile can be made. A browser installed as a snap
/// may not write to hidden folders in the home folder, where the app's own
/// folder is on Linux, so it is given a place in its own snap folder.
fn profile_base(scratch: &Path, browser: &Path) -> PathBuf {
    snap_name(browser)
        .zip(std::env::var_os("HOME"))
        .map(|(name, home)| Path::new(&home).join("snap").join(name).join("common"))
        .filter(|base| base.is_dir())
        .unwrap_or_else(|| scratch.to_owned())
}

/// The snap a browser comes from: `/snap/bin/chromium` or
/// `/snap/chromium/current/...` are both `chromium`.
fn snap_name(browser: &Path) -> Option<String> {
    let rest = browser.strip_prefix("/snap/").ok()?;
    let mut parts = rest.components().map(|part| part.as_os_str().to_str());
    match parts.next()?? {
        "bin" => parts.next()??.split('.').next().map(str::to_owned),
        name => Some(name.to_owned()),
    }
}

/// Throwaway profiles left by a sign-in the app was closed during. Each
/// holds a session, so none is kept.
fn forget_profiles(base: &Path) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(PROFILE_PREFIX))
        {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// The sign-in page in an ordinary window, started.
fn open_sign_in(browser: &Path, profile: &Path, url: &str) -> Result<Child, SignInError> {
    Ok(Command::new(browser)
        .arg(format!("--user-data-dir={}", profile.display()))
        .args(STORE_FLAGS)
        .args([
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-sync",
            "--new-window",
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?)
}

/// Step one: the sign-in page, until the browser has been closed, or told to
/// close because the sign-in has landed, or the sign-in is called off.
fn show_sign_in(
    browser: &Path,
    profile: &Path,
    control: &Control,
    url: &str,
) -> Result<(), SignInError> {
    let mut window = open_sign_in(browser, profile, url)?;
    let deadline = Instant::now() + SIGN_IN_MOST;
    let mut watch = Watch::new(profile);
    loop {
        if window.try_wait()?.is_some() {
            return Ok(());
        }
        if control.cancelled() {
            let _ = window.kill();
            let _ = window.wait();
            return Err(SignInError::Cancelled);
        }
        if Instant::now() >= deadline {
            let _ = window.kill();
            let _ = window.wait();
            return Err(SignInError::TimedOut);
        }
        #[cfg(windows)]
        if control.take_raise() {
            windows::raise(window.id());
        }
        if watch.signed_in(window.id()) {
            stop_browser(&mut window);
            return Ok(());
        }
        std::thread::sleep(LOOK_EVERY);
    }
}

/// What tells that the sign-in has landed.
struct Watch {
    #[cfg(unix)]
    profile: PathBuf,
    #[cfg(unix)]
    last_read: Option<Instant>,
}

impl Watch {
    #[cfg(unix)]
    fn new(profile: &Path) -> Self {
        Watch {
            profile: profile.to_owned(),
            last_read: None,
        }
    }

    #[cfg(windows)]
    fn new(_profile: &Path) -> Self {
        Watch {}
    }

    /// The profile's cookie file names the cookie YouTube leaves on a
    /// sign-in.
    #[cfg(unix)]
    fn signed_in(&mut self, _browser: u32) -> bool {
        if self
            .last_read
            .is_some_and(|read| read.elapsed() < READ_COOKIES_EVERY)
        {
            return false;
        }
        self.last_read = Some(Instant::now());
        COOKIE_FILES
            .iter()
            .any(|file| has_sign_in_cookie(&self.profile.join(file)))
    }

    /// One of the browser's windows says YouTube Music has loaded. Two
    /// seconds' grace then lets the browser finish writing its cookies.
    #[cfg(windows)]
    fn signed_in(&mut self, browser: u32) -> bool {
        if windows::titles(browser)
            .iter()
            .any(|title| title.contains("YouTube Music"))
        {
            std::thread::sleep(Duration::from_secs(2));
            return true;
        }
        false
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
fn stop_browser(browser: &mut Child) {
    #[cfg(unix)]
    {
        let told = Command::new("kill")
            .args(["-TERM", &browser.id().to_string()])
            .status();
        if let Err(error) = told {
            log::debug!("the browser could not be told to leave: {error}");
        }
    }
    #[cfg(windows)]
    windows::close(browser.id());
    let deadline = Instant::now() + BROWSER_STOP;
    while Instant::now() < deadline {
        if !matches!(browser.try_wait(), Ok(None)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    log::debug!("the browser did not leave when asked; ending it");
    let _ = browser.kill();
    let _ = browser.wait();
}

/// Waits for the browser to let go of the profile, which its helper
/// processes can hold a moment after the browser itself has gone. Reopened
/// before then, the profile would be refused as in use.
fn wait_for_release(profile: &Path, control: &Control) -> Result<(), SignInError> {
    let deadline = Instant::now() + PROFILE_RELEASE;
    while profile_in_use(profile) && Instant::now() < deadline {
        control.check()?;
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// A running browser keeps `SingletonLock`, a link to its host and process,
/// in its profile, and removes it as it leaves.
#[cfg(unix)]
fn profile_in_use(profile: &Path) -> bool {
    std::fs::symlink_metadata(profile.join("SingletonLock")).is_ok()
}

/// A running browser holds `lockfile` open in its profile, so it cannot be
/// removed until the browser has gone; once it can, the profile is free.
#[cfg(windows)]
fn profile_in_use(profile: &Path) -> bool {
    let lock = profile.join("lockfile");
    lock.exists() && std::fs::remove_file(&lock).is_err()
}

/// The browser's windows, read and handled the way the taskbar does:
/// nothing touches the page.
#[cfg(windows)]
mod windows {
    use ::windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use ::windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        PostMessageW, SW_RESTORE, SetForegroundWindow, ShowWindow, WM_CLOSE,
    };
    use ::windows::core::BOOL;

    struct Search {
        pid: u32,
        found: Vec<HWND>,
    }

    unsafe extern "system" fn each(window: HWND, search: LPARAM) -> BOOL {
        // SAFETY: `search` is the `Search` in `windows_of`, alive for the call.
        let search = unsafe { &mut *(search.0 as *mut Search) };
        let mut owner = 0u32;
        // SAFETY: `window` was handed over by the system just now.
        unsafe { GetWindowThreadProcessId(window, Some(&raw mut owner)) };
        // SAFETY: as above.
        if owner == search.pid && unsafe { IsWindowVisible(window) }.as_bool() {
            search.found.push(window);
        }
        BOOL::from(true)
    }

    /// The visible top-level windows of process `pid`.
    fn windows_of(pid: u32) -> Vec<HWND> {
        let mut search = Search {
            pid,
            found: Vec::new(),
        };
        // SAFETY: the callback only touches `search`, which outlives the call.
        let _ = unsafe { EnumWindows(Some(each), LPARAM(&raw mut search as isize)) };
        search.found
    }

    /// What the browser's windows are titled.
    pub(super) fn titles(pid: u32) -> Vec<String> {
        windows_of(pid)
            .into_iter()
            .map(|window| {
                let mut text = [0u16; 512];
                // SAFETY: the buffer is ours, and its length goes with it.
                let length = unsafe { GetWindowTextW(window, &mut text) };
                String::from_utf16_lossy(&text[..usize::try_from(length).unwrap_or(0)])
            })
            .collect()
    }

    /// Closes the browser's windows as their X would.
    pub(super) fn close(pid: u32) {
        for window in windows_of(pid) {
            // SAFETY: a message to a window that has gone is simply lost.
            let _ = unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
    }

    /// Brings the browser's window to the front. The system allows it, as
    /// the app asking is the one in front.
    pub(super) fn raise(pid: u32) {
        if let Some(&window) = windows_of(pid).first() {
            // SAFETY: as above.
            unsafe {
                if IsIconic(window).as_bool() {
                    let _ = ShowWindow(window, SW_RESTORE);
                }
                let _ = SetForegroundWindow(window);
            }
        }
    }
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
            return Err(SignInError::Failed(
                "the browser did not start to hand the sign-in over".into(),
            ));
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
    use crate::scratch::Scratch;

    #[cfg(unix)]
    #[test]
    fn the_cookie_a_sign_in_leaves_is_found_by_its_name() {
        let dir = Scratch::folder(&format!("spotified-signin-{}", std::process::id()));
        let file = dir.join("Cookies");
        assert!(!has_sign_in_cookie(&file), "no file, no sign-in");
        std::fs::write(&file, b"SQLite format 3 ...SID...").expect("write");
        assert!(!has_sign_in_cookie(&file));
        std::fs::write(&file, b"SQLite format 3 ...LOGIN_INFO...").expect("write");
        assert!(has_sign_in_cookie(&file));
    }

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

    #[test]
    fn a_snapped_browser_is_known_by_its_snap() {
        assert_eq!(
            snap_name(Path::new("/snap/bin/chromium")).as_deref(),
            Some("chromium")
        );
        assert_eq!(
            snap_name(Path::new("/snap/bin/brave.browser")).as_deref(),
            Some("brave")
        );
        assert_eq!(
            snap_name(Path::new("/snap/chromium/current/usr/lib/chromium")).as_deref(),
            Some("chromium")
        );
        assert_eq!(snap_name(Path::new("/usr/bin/google-chrome")), None);
    }

    #[test]
    fn profiles_left_by_an_earlier_sign_in_are_removed_and_nothing_else() {
        let base = Scratch::folder(&format!("spotified-profiles-{}", std::process::id()));
        std::fs::create_dir_all(base.join("signin-browser")).expect("old profile");
        std::fs::create_dir_all(base.join("signin-browser-123")).expect("left profile");
        std::fs::create_dir_all(base.join("accounts")).expect("other folder");
        forget_profiles(&base);
        assert!(!base.join("signin-browser").exists());
        assert!(!base.join("signin-browser-123").exists());
        assert!(base.join("accounts").exists());
    }

    #[test]
    fn a_cancelled_sign_in_says_so_before_doing_anything_more() {
        let control = Control::default();
        assert!(control.check().is_ok());
        control.cancel();
        assert!(matches!(control.check(), Err(SignInError::Cancelled)));
    }

    /// Opens a real browser on YouTube Music, signed out, which is the page
    /// a sign-in lands on: the window is noticed, closed, and its profile
    /// read back, short of there being anyone signed in.
    #[test]
    #[ignore = "opens the installed browser for a few seconds; run with --ignored"]
    fn the_landing_page_is_noticed_and_its_window_closed_and_read() {
        let browser = browser().expect("a browser");
        let profile = Scratch::path("spotified-signin-landing");
        let _ = std::fs::remove_dir_all(&profile);
        let started = Instant::now();
        show_sign_in(
            &browser,
            &profile,
            &Control::default(),
            "https://music.youtube.com/",
        )
        .expect("the window noticed and closed");
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "noticed in time"
        );
        wait_for_release(&profile, &Control::default()).expect("released");
        let cookies = read_cookies(&browser, &profile).expect("read back");
        assert!(!cookies.is_empty(), "YouTube left its visitor cookies");
        assert_eq!(cookie_header(&cookies), None, "nobody is signed in");
        let _ = std::fs::remove_dir_all(&profile);
    }

    /// A sign-in called off closes its window at once.
    #[test]
    #[ignore = "opens the installed browser for a moment; run with --ignored"]
    fn a_cancelled_sign_in_closes_its_window() {
        let browser = browser().expect("a browser");
        let profile = Scratch::path("spotified-signin-cancel");
        let control = std::sync::Arc::new(Control::default());
        let canceller = control.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(3));
            canceller.cancel();
        });
        let started = Instant::now();
        let result = show_sign_in(&browser, &profile, &control, "about:blank");
        assert!(matches!(result, Err(SignInError::Cancelled)));
        assert!(started.elapsed() < Duration::from_secs(5));
        wait_for_release(&profile, &Control::default()).expect("released");
        let _ = std::fs::remove_dir_all(&profile);
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

//! What must never reach the log, or a problem report.
//!
//! The session cookies are the account; a googlevideo address is signed
//! for this computer's own address and carries a token. E-mail addresses
//! and the name of the Windows user folder are not secrets, but they are
//! not ours to collect either. Song titles and video ids stay: without
//! them a log cannot say which song failed.
//!
//! Lines are scrubbed on their way into the log, not when a report is
//! made: a log that holds cookies on disk is a liability whether or not
//! anyone ever sends it. The patterns are the Electron app's, one for one.

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

/// Each pattern and what a match becomes, in the order they are applied.
const RULES: [(&str, &str); 9] = [
    // A header or field that carries the session, to the end of its line.
    (
        r#"(?i)\b(cookie|set-cookie|authorization|x-goog-authuser|x-goog-visitor-id|x-youtube-identity-token)(["']?\s*[:=]\s*)[^\r\n]+"#,
        "${1}${2}<redacted>",
    ),
    (r"SAPISID(?:1P|3P)?HASH\s+\S+", "SAPISIDHASH <redacted>"),
    // The session's cookies by name, wherever they turn up.
    (
        r#"\b(__Secure-[\w-]+|__Host-[\w-]+|SID|HSID|SSID|APISID|SAPISID|SIDCC|LOGIN_INFO|NID|VISITOR_INFO1_LIVE|VISITOR_PRIVACY_METADATA|YSC|PREF)=[^;\s"',]+"#,
        "${1}=<redacted>",
    ),
    // A line of a cookie file: domain, flag, path, secure, expiry, name,
    // and then the value.
    (
        r"(?i)(\.(?:youtube|google)\.com\t(?:[^\t\r\n]*\t){5})[^\t\r\n]+",
        "${1}<redacted>",
    ),
    (
        r#"https?://[^\s"'<>]*googlevideo\.com[^\s"'<>]*"#,
        "https://…googlevideo.com/<redacted>",
    ),
    (
        r#"(?i)([?&](?:sig|signature|lsig|pot|key|access_token|token|visitorData)=)[^&\s"']+"#,
        "${1}<redacted>",
    ),
    (
        r#"(?i)("(?:visitorData|poToken|accessToken|refreshToken|sapisid)"\s*:\s*")[^"]*""#,
        "${1}<redacted>\"",
    ),
    (r"[\w.+-]+@[\w-]+(?:\.[\w-]+)+", "<email>"),
    (r#"([A-Za-z]:[\\/]+Users[\\/]+)[^\\/\s"']+"#, "${1}<user>"),
];

/// The patterns, compiled once. One that did not compile would be a bug in
/// this file, which the tests below would have caught; it is left out
/// rather than taking the app down.
static COMPILED: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    RULES
        .iter()
        .filter_map(|(pattern, becomes)| Some((Regex::new(pattern).ok()?, *becomes)))
        .collect()
});

/// `text` with everything that must not be kept taken out. Borrowed back
/// unchanged, without an allocation, when there was nothing to take.
pub fn redact(text: &str) -> Cow<'_, str> {
    let mut out = Cow::Borrowed(text);
    for (pattern, becomes) in COMPILED.iter() {
        if let Cow::Owned(changed) = pattern.replace_all(&out, *becomes) {
            out = Cow::Owned(changed);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_compiles() {
        assert_eq!(COMPILED.len(), RULES.len());
    }

    #[test]
    fn a_cookie_header_loses_everything_after_its_name() {
        assert_eq!(redact("Cookie: SID=abc; HSID=def"), "Cookie: <redacted>");
        assert_eq!(
            redact(r#"{"authorization": "SAPISIDHASH 123_abc"}"#),
            r#"{"authorization": <redacted>"#
        );
        assert_eq!(
            redact("x-goog-authuser=0 and more"),
            "x-goog-authuser=<redacted>"
        );
    }

    #[test]
    fn a_request_signature_is_taken_whichever_cookie_it_was_made_from() {
        assert_eq!(
            redact("sent SAPISID1PHASH 1700000000_deadbeef to youtube"),
            "sent SAPISIDHASH <redacted> to youtube"
        );
    }

    #[test]
    fn session_cookies_are_taken_by_name_and_other_pairs_are_left() {
        assert_eq!(
            redact("jar: SAPISID=secret; theme=dark; __Secure-3PSID=also"),
            "jar: SAPISID=<redacted>; theme=dark; __Secure-3PSID=<redacted>"
        );
        // A word that merely ends in a cookie's name is not one.
        assert_eq!(redact("VALID=yes"), "VALID=yes");
    }

    #[test]
    fn a_line_of_a_cookie_file_keeps_its_name_and_loses_its_value() {
        let line = ".youtube.com\tTRUE\t/\tTRUE\t1790000000\tLOGIN_INFO\tAFmmF2swRQ";
        assert_eq!(
            redact(line),
            ".youtube.com\tTRUE\t/\tTRUE\t1790000000\tLOGIN_INFO\t<redacted>"
        );
    }

    #[test]
    fn a_stream_address_is_taken_whole() {
        let line = "playing https://rr3---sn-abc.googlevideo.com/videoplayback?expire=1&ip=1.2.3.4&sig=x ok";
        assert_eq!(
            redact(line),
            "playing https://…googlevideo.com/<redacted> ok"
        );
    }

    #[test]
    fn tokens_in_an_address_or_in_json_are_taken_and_the_rest_is_left() {
        assert_eq!(
            redact("GET /v1/x?id=abc&pot=SECRET&n=1"),
            "GET /v1/x?id=abc&pot=<redacted>&n=1"
        );
        assert_eq!(
            redact(r#"{"visitorData":"Cgt4","videoId":"dQw4w9WgXcQ"}"#),
            r#"{"visitorData":"<redacted>","videoId":"dQw4w9WgXcQ"}"#
        );
    }

    #[test]
    fn an_email_address_and_the_windows_user_are_not_collected() {
        assert_eq!(
            redact("signed in as ada.lovelace+music@example.co.uk"),
            "signed in as <email>"
        );
        assert_eq!(
            redact(r"cache at C:\Users\ada\AppData\Local\SpotifiedNative\cache"),
            r"cache at C:\Users\<user>\AppData\Local\SpotifiedNative\cache"
        );
        assert_eq!(
            redact("C:/Users/ada/Downloads/report.zip"),
            "C:/Users/<user>/Downloads/report.zip"
        );
    }

    #[test]
    fn what_a_log_needs_is_left_alone() {
        let line = "track dQw4w9WgXcQ \"Never Gonna Give You Up\" failed: HTTP Error 403";
        assert!(matches!(redact(line), Cow::Borrowed(_)));
    }
}

package dev.darksavci.spotified.data

/**
 * What must never leave the phone in a log or a problem report: the session's
 * cookies, a stream address signed for this phone, and e-mail addresses. Song
 * titles and video ids stay, since without them a log cannot say which song
 * failed. The patterns are the desktop's (crates/app/src/redact.rs), one for one.
 */
object Redact {
    private val RULES: List<Pair<Regex, String>> = listOf(
        // A header or field that carries the session, to the end of its line.
        Regex("""(?i)\b(cookie|set-cookie|authorization|x-goog-authuser|x-goog-visitor-id|x-youtube-identity-token)(["']?\s*[:=]\s*)[^\r\n]+""") to "$1$2<redacted>",
        Regex("""SAPISID(?:1P|3P)?HASH\s+\S+""") to "SAPISIDHASH <redacted>",
        // The session's cookies by name, wherever they turn up.
        Regex("""\b(__Secure-[\w-]+|__Host-[\w-]+|SID|HSID|SSID|APISID|SAPISID|SIDCC|LOGIN_INFO|NID|VISITOR_INFO1_LIVE|VISITOR_PRIVACY_METADATA|YSC|PREF)=[^;\s"',]+""") to "$1=<redacted>",
        // A line of a cookie file: domain, flag, path, secure, expiry, name, and then the value.
        Regex("""(?i)(\.(?:youtube|google)\.com\t(?:[^\t\r\n]*\t){5})[^\t\r\n]+""") to "$1<redacted>",
        Regex("""https?://[^\s"'<>]*googlevideo\.com[^\s"'<>]*""") to "https://…googlevideo.com/<redacted>",
        Regex("""(?i)([?&](?:sig|signature|lsig|pot|key|access_token|token|visitorData)=)[^&\s"']+""") to "$1<redacted>",
        Regex("""(?i)("(?:visitorData|poToken|accessToken|refreshToken|sapisid)"\s*:\s*")[^"]*"""") to "$1<redacted>\"",
        Regex("""[\w.+-]+@[\w-]+(?:\.[\w-]+)+""") to "<email>",
    )

    /** [text] with everything that must not be sent taken out. */
    fun text(text: String): String = RULES.fold(text) { out, (pattern, becomes) -> pattern.replace(out, becomes) }
}

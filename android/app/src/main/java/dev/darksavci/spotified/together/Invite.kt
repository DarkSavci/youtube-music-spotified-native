package dev.darksavci.spotified.together

import java.net.URLDecoder
import java.net.URLEncoder

/*
 * An invitation to a room, as a link: spotified://join?pin=01234567&server=wss%3A%2F%2Flisten.example.com
 *
 * A link of the app's own scheme, because no web address belongs to the app:
 * since Android 12 an https link that a site has not verified opens in the
 * browser whatever the app declares. The PIN comes first so that the desktop's
 * PIN field, which keeps the first eight digits of what is pasted into it, still
 * finds the right one; the server's address could hold digits of its own.
 *
 * A link can come from anyone, and following it tells the server who this is, so
 * it never joins by itself: the page asks first, naming the server.
 */

/** A room to join and the server it is on. */
data class Invite(val pin: String, val server: String)

private const val JOIN_LINK = "spotified://join"
private const val BAD_INVITE = "This invite link is not complete. Ask for a new one, or enter the PIN yourself."
private val WHOLE_PIN = Regex("""\d{8}""")
private val INVITE = Regex("""spotified://join\?\S+""", RegexOption.IGNORE_CASE)

/** The longest server address the form takes. */
private const val ADDRESS_MOST = 300

/**
 * The link to a room, if it can have one. Only a secure server can: `ws://` is
 * for a relay on this device, which is no use to anyone else.
 */
fun inviteLink(server: String, pin: String): String? {
    val address = checkedAddress(server).getOrNull()?.takeIf { it.startsWith("wss://") } ?: return null
    if (!WHOLE_PIN.matches(pin)) return null
    return "$JOIN_LINK?pin=$pin&server=${URLEncoder.encode(address, "UTF-8")}"
}

/** What is shared to invite someone: the link, and the way in for whoever cannot open it. */
fun inviteText(room: String, server: String, pin: String): String {
    val by = "enter PIN ${spacedPin(pin)} in Listen Together, on the server $server."
    val link = inviteLink(server, pin) ?: return "Join “$room” on Listen Together: $by"
    return "Join “$room” on Listen Together.\n\n$link\n\nIf the link doesn’t open, $by"
}

/** The invitation in [text], or why what looks like one cannot be used. */
fun readInvite(text: String): Result<Invite> {
    val link = INVITE.find(text)?.value ?: return refusedInvite(BAD_INVITE)
    val fields = runCatching {
        link.substringAfter('?').trimEnd('.', ',', ')', '>', '"', '\'', '”').split('&').associate { field ->
            field.substringBefore('=') to URLDecoder.decode(field.substringAfter('=', ""), "UTF-8")
        }
    }.getOrElse { return refusedInvite(BAD_INVITE) }
    val pin = fields["pin"]?.takeIf(WHOLE_PIN::matches) ?: return refusedInvite(BAD_INVITE)
    val server = fields["server"]?.trim()?.takeIf { it.isNotEmpty() && it.length <= ADDRESS_MOST } ?: return refusedInvite(BAD_INVITE)
    if (server.any(Char::isWhitespace)) return refusedInvite(BAD_INVITE)
    val address = checkedAddress(server).getOrElse { return refusedInvite("This invite link’s server can’t be used. ${it.message.orEmpty()}") }
    // Allowed when typed in by hand for testing, but a link must not reach into this device.
    if (!address.startsWith("wss://")) return refusedInvite("This invite link’s server isn’t secure, so it wasn’t opened.")
    return Result.success(Invite(pin, address))
}

private fun refusedInvite(why: String): Result<Invite> = Result.failure(IllegalArgumentException(why))

/** The saved server this invitation is for, if there is one. */
fun Invite.savedIn(servers: List<SavedServer>): SavedServer? =
    servers.firstOrNull { it.url.trim().trimEnd('/') == server.trimEnd('/') }

/** The servers with this invitation's among them, and which it is. */
fun Invite.withServerIn(servers: List<SavedServer>): Pair<List<SavedServer>, SavedServer>? {
    savedIn(servers)?.let { return servers to it }
    val (saved, id) = save(servers, ServerForm(url = server)).getOrNull() ?: return null
    return saved.firstOrNull { it.id == id }?.let { saved to it }
}

/** What is asked before following an invitation. [saved] is the server's name if it is known; [leaving] the room this listener would leave. */
fun inviteQuestion(invite: Invite, saved: String?, leaving: String?): String {
    val pin = spacedPin(invite.pin)
    val where = if (saved != null) {
        "This is room $pin on $saved."
    } else {
        "This is room $pin on ${hostOf(invite.server)}, a server you haven’t used. Whoever runs it can see your name, " +
            "your picture if you share it, and your internet address, so join only if you trust who sent this."
    }
    return if (leaving != null) "$where You’ll leave “$leaving” first." else where
}

/** The PIN as it is read out: two groups of four. */
fun spacedPin(pin: String) = if (pin.length == 8) "${pin.take(4)} ${pin.drop(4)}" else pin

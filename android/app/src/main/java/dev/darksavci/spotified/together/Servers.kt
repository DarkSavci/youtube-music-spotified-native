package dev.darksavci.spotified.together

import kotlinx.serialization.Serializable

/*
 * The relays a listener has saved: a name for each, so friends can agree on
 * "the same server" without reading an address out. Mirrors
 * crates/app/src/together/servers.rs.
 */

@Serializable
data class SavedServer(val id: String = "", val name: String = "", val url: String = "")

/** The form that adds a server or edits one. */
data class ServerForm(
    /** The server being edited; null for a new one. */
    val editing: String? = null,
    val name: String = "",
    val url: String = "",
    /** What testing the address came to, or that it is being tested. */
    val check: String = "",
) {
    companion object {
        /** The form as it opens on [server]: its own name and address, or empty for a new one. */
        fun on(server: SavedServer?): ServerForm =
            server?.let { ServerForm(editing = it.id, name = it.name, url = it.url) } ?: ServerForm()
    }
}

/** What the form's test says while it waits for the server. */
const val CHECKING = "Checking…"
const val CHECKED = "Ready for v2 rooms"

/** The host of a checked address, which names a server that was given no name. */
internal fun hostOf(address: String): String = address.substringAfter("://").substringBefore('/')

/** An id no saved server has yet. */
private fun freshId(servers: List<SavedServer>): String {
    val highest = servers.mapNotNull { server ->
        server.id.takeIf { it.startsWith("server-") }?.removePrefix("server-")?.toLongOrNull()
    }.maxOrNull()
    return "server-${(highest ?: 0) + 1}"
}

/**
 * Saves what the form holds, as a new server or over the one it edits:
 * the servers as they then are, and its id. The address must be one a room
 * could be reached at.
 */
fun save(servers: List<SavedServer>, form: ServerForm): Result<Pair<List<SavedServer>, String>> {
    val url = checkedAddress(form.url).getOrElse { return Result.failure(it) }
    val name = form.name.trim().ifEmpty { hostOf(url) }
    val id = form.editing ?: freshId(servers)
    val saved = servers.filter { it.id != id } + SavedServer(id, name, url)
    return Result.success(saved to id)
}

/**
 * The digits of a room's PIN in what was typed or pasted: the PIN itself,
 * spaced or not, or a link that carries one.
 */
fun pinFrom(text: String): String {
    val named = Regex("""pin=(\d{8})""").find(text)?.groupValues?.get(1)
    return named ?: text.filter(Char::isDigit).take(8)
}

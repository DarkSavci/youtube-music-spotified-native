package dev.darksavci.spotified.data

import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.Track
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/**
 * The account's liked songs, by id, for the heart on every row and in the
 * notification. A like shows at once; a write that fails flips it back and
 * says so, and reads while a write is in flight do not undo it. They belong to
 * one account: another account's start empty, and a read that fails leaves them
 * as they were only if they are still that account's to show.
 */
class Liked(
    private val fetch: suspend () -> List<String>,
    private val write: suspend (id: String, liked: Boolean) -> Unit,
    /** The account in use. */
    private val account: () -> String,
    private val scope: CoroutineScope,
) {
    private val _ids = MutableStateFlow<Set<String>>(emptySet())
    val ids: StateFlow<Set<String>> = _ids

    private val _failures = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val failures: SharedFlow<String> = _failures

    private val pending = mutableMapOf<String, Boolean>()
    /** Whose likes these are, and the read whose answer is awaited: an older one's is out of date. */
    private var owner: String? = null
    private var reads = 0

    private fun forget() {
        synchronized(pending) { pending.clear() }
        _ids.value = emptySet()
    }

    fun refresh() {
        val asked = account()
        val read = synchronized(pending) {
            if (owner != asked) {
                owner = asked
                pending.clear()
                _ids.value = emptySet()
            }
            ++reads
        }
        scope.launch {
            val result = runCatching { fetch() }
            val current = synchronized(pending) { read == reads && owner == asked }
            if (!current) return@launch
            result.onSuccess { list ->
                val fresh = list.toMutableSet()
                synchronized(pending) {
                    pending.forEach { (id, liked) -> if (liked) fresh += id else fresh -= id }
                }
                _ids.value = fresh
            }.onFailure { e ->
                // Signed out, they are no longer anyone's. Any other failure says nothing of them.
                if (e is ApiError.SignedOut) forget()
            }
        }
    }

    fun isLiked(id: String) = id in _ids.value

    fun toggle(track: Track) = set(track.id, !isLiked(track.id))

    fun set(id: String, liked: Boolean) {
        if (id.isEmpty()) return
        val whose = synchronized(pending) {
            pending[id] = liked
            owner
        }
        _ids.update { if (liked) it + id else it - id }
        scope.launch {
            val ok = runCatching { write(id, liked) }
            val same = synchronized(pending) {
                pending.remove(id)
                owner == whose
            }
            ok.onFailure { e ->
                if (same) _ids.update { if (liked) it - id else it + id }
                _failures.tryEmit(e.message ?: "That could not be saved.")
            }
        }
    }
}

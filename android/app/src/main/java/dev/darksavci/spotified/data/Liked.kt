package dev.darksavci.spotified.data

import dev.darksavci.spotified.api.Api
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
 * says so, and reads while a write is in flight do not undo it.
 */
class Liked(private val api: Api, private val scope: CoroutineScope) {
    private val _ids = MutableStateFlow<Set<String>>(emptySet())
    val ids: StateFlow<Set<String>> = _ids

    private val _failures = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val failures: SharedFlow<String> = _failures

    private val pending = mutableMapOf<String, Boolean>()

    fun refresh() {
        scope.launch {
            runCatching { api.liked() }.onSuccess { list ->
                val fresh = list.tracks.map { it.id }.toMutableSet()
                synchronized(pending) {
                    pending.forEach { (id, liked) -> if (liked) fresh += id else fresh -= id }
                }
                _ids.value = fresh
            }
        }
    }

    fun isLiked(id: String) = id in _ids.value

    fun toggle(track: Track) = set(track.id, !isLiked(track.id))

    fun set(id: String, liked: Boolean) {
        if (id.isEmpty()) return
        synchronized(pending) { pending[id] = liked }
        _ids.update { if (liked) it + id else it - id }
        scope.launch {
            val ok = runCatching { api.setLiked(id, liked) }
            synchronized(pending) { pending.remove(id) }
            ok.onFailure { e ->
                _ids.update { if (liked) it - id else it + id }
                _failures.tryEmit(e.message ?: "That could not be saved.")
            }
        }
    }
}

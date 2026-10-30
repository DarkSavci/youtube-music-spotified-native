package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.ArtistRef
import dev.darksavci.spotified.api.BrowsePage
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryItem
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.SearchResults
import dev.darksavci.spotified.api.Shelf
import dev.darksavci.spotified.api.Track

/*
 * What the browsing pages work out, away from the pages, so that it can be tested.
 */

// ---------- recent searches ----------

internal const val RECENT_SEARCHES_KEPT = 20

/** [query] put first in [recent], once however it was written. Empty text changes nothing. */
internal fun rememberedSearch(recent: List<String>, query: String): List<String> {
    val q = query.trim()
    if (q.isEmpty()) return recent
    return (listOf(q) + recent.filterNot { it.equals(q, ignoreCase = true) }).take(RECENT_SEARCHES_KEPT)
}

internal fun forgottenSearch(recent: List<String>, query: String): List<String> =
    recent.filterNot { it.equals(query.trim(), ignoreCase = true) }

// ---------- the library ----------

enum class LibrarySort(val label: String) { Recent("Recents"), Added("Recently added"), Alphabetical("Alphabetical") }

/**
 * What the library page lists. Inside a folder, what is in it. At the top, what
 * is in no folder, since the folders stand for the rest; a kind or a search looks
 * through everything, folders or not. Pinned items come first, whatever the sort.
 */
internal fun libraryShown(
    items: List<LibraryItem>,
    kind: LibraryKind?,
    folder: String?,
    query: String,
    sort: LibrarySort,
): List<LibraryItem> {
    val q = query.trim()
    return items
        .filter { kind == null || it.kind == kind }
        .filter {
            when {
                folder != null -> it.folderId == folder
                kind != null || q.isNotEmpty() -> true
                else -> it.folderId.isEmpty()
            }
        }
        .filter { q.isEmpty() || it.title.contains(q, ignoreCase = true) || it.subtitle.contains(q, ignoreCase = true) }
        .let { list ->
            when (sort) {
                LibrarySort.Recent -> list.sortedByDescending { it.lastPlayedAt ?: it.addedAt ?: "" }
                LibrarySort.Added -> list.sortedByDescending { it.addedAt ?: "" }
                LibrarySort.Alphabetical -> list.sortedBy { it.title.lowercase() }
            }
        }
        .sortedByDescending { it.pinned }
}

/** A playlist's id without the "VL" of its browse id, as the library may write it either way. */
internal fun bareId(id: String): String = id.removePrefix("VL")

/** The library's line for this page, if it is in the library. */
internal fun libraryEntry(items: List<LibraryItem>, kind: LibraryKind, id: String): LibraryItem? =
    items.firstOrNull { it.kind == kind && bareId(it.id) == bareId(id) }

// ---------- what is playing ----------

/**
 * Whether the queue is this list, by what it holds rather than only what it is
 * called: two albums can share a title. The queue has this list's name, is on one
 * of its songs, and holds at least half of them, so a song or two taken out of the
 * queue, or songs added after it, leave it this list.
 */
internal fun isQueueOf(queue: List<Track>, index: Int, queueOrigin: String, tracks: List<Track>, origin: String): Boolean {
    if (queueOrigin != origin) return false
    val current = queue.getOrNull(index)?.id ?: return false
    val ids = tracks.filter { it.playable && it.id.isNotEmpty() }.map { it.id }.toSet()
    if (ids.isEmpty() || current !in ids) return false
    val held = queue.mapTo(HashSet()) { it.id }
    return ids.count { it in held } * 2 >= ids.size
}

// ---------- lists of songs ----------

enum class SongSort(val label: String) {
    Custom("Custom order"), Title("Title"), Artist("Artist"), Album("Album"), Duration("Duration")
}

/** The songs in the order chosen to view them; the list itself keeps its order. */
internal fun sortedSongs(tracks: List<Track>, sort: SongSort): List<Track> = when (sort) {
    SongSort.Custom -> tracks
    SongSort.Title -> tracks.sortedBy { it.title.lowercase() }
    SongSort.Artist -> tracks.sortedBy { it.artistNames.lowercase() }
    SongSort.Album -> tracks.sortedBy { it.album?.name.orEmpty().lowercase() }
    SongSort.Duration -> tracks.sortedBy { it.durationMs }
}

/**
 * A key for each row. A song can be in a list twice: its place's id tells them
 * apart where there is one, and otherwise which time it is that it appears.
 */
internal fun rowKeys(tracks: List<Track>): List<String> {
    val seen = HashMap<String, Int>()
    return tracks.map { t ->
        val base = t.playlistItemId.ifEmpty { t.id }
        val n = seen.getOrDefault(base, 0)
        seen[base] = n + 1
        if (t.playlistItemId.isNotEmpty() && n == 0) base else "$base#$n"
    }
}

/** The chosen songs, in the list's order. */
internal fun chosenSongs(tracks: List<Track>, keys: List<String>, chosen: Set<String>): List<Track> =
    tracks.filterIndexed { i, _ -> keys.getOrNull(i) in chosen }

/** The likes a change of [before] to [after] took away, added to [gone], less those given back. */
internal fun unlikedSince(gone: Set<String>, before: Set<String>, after: Set<String>): Set<String> =
    (gone + (before - after)) - after

/**
 * The liked page's rows, less what was unliked since it loaded. By what has been
 * taken away, not by what the like list holds: the page can be newer than that
 * list, and a song liked elsewhere must not be hidden for it.
 */
internal fun stillLiked(tracks: List<Track>, unliked: Set<String>): List<Track> =
    if (unliked.isEmpty()) tracks else tracks.filter { it.id !in unliked }

// ---------- music videos ----------

/** A shelf without its music videos, unless they are asked for. As on the desktop, an album or a playlist always shows all it holds. */
internal fun shelfShown(shelf: Shelf, videos: Boolean): Shelf =
    if (videos) shelf else shelf.copy(items = shelf.items.filterNot { it is Item.Song && it.track.isVideo })

internal fun songsShown(tracks: List<Track>, videos: Boolean): List<Track> =
    if (videos) tracks else tracks.filterNot { it.isVideo }

// ---------- search ----------

/** What sits under the top result. Songs under an artist often come without the artist's name: it is the top result's. */
internal fun topResultBeside(results: SearchResults): List<Item> {
    val by = (results.topResult as? Item.ArtistItem)?.artist
    return results.topResultItems.map { item ->
        if (item is Item.Song && item.track.artists.isEmpty() && by != null) {
            Item.Song(item.track.copy(artists = listOf(ArtistRef(by.id, by.name))))
        } else item
    }
}

/** The songs of [tracks] asked to download that are not whole yet: what a batch under way still has to do. */
internal fun downloadsComing(tracks: List<Track>, wanted: Set<String>, done: Set<String>): List<Track> =
    tracks.filter { it.id in wanted && it.id !in done }

/** Where a song listed is in the queue it plays: by its id, for a row may carry more than the queue's copy. */
internal fun placeInQueue(songs: List<Track>, track: Track): Int = songs.indexOfFirst { it.id == track.id }.coerceAtLeast(0)

/** What a song in Top result plays: itself, then the songs beside it, then the songs below, each once. */
internal fun topResultQueue(results: SearchResults): List<Track> {
    val top = (results.topResult as? Item.Song)?.track
    val beside = topResultBeside(results).filterIsInstance<Item.Song>().map { it.track }
    val below = results.shelves.firstOrNull { s -> s.items.any { it is Item.Song } }
        ?.items.orEmpty().filterIsInstance<Item.Song>().map { it.track }
    return (listOfNotNull(top) + beside + below).distinctBy { it.id }
}

// ---------- home ----------

/** Whether a country has the podcasts page: null when asking told nothing, which is asked again. */
internal fun podcastsAnswer(result: Result<BrowsePage>): Boolean? = result.fold(
    { page -> page.shelves.isNotEmpty() },
    { e -> if (e is ApiError.Status && e.code == 404) false else null },
)

/** The chips to keep showing: a mood's page can answer with others, or none; All's are the row. */
internal fun moodsAfter(old: List<dev.darksavci.spotified.api.HomeChip>, page: BrowsePage, onAll: Boolean) =
    if (page.chips.isNotEmpty() && (onAll || old.isEmpty())) page.chips else old

// ---------- browse ----------

/**
 * A "Show all" page is one shelf with the page's own name, so the name is said
 * once: the shelf's heading goes when it only repeats the bar's.
 */
internal fun shelfHeading(shelf: Shelf, pageTitle: String): String =
    shelf.title.takeUnless { it.equals(pageTitle, ignoreCase = true) }.orEmpty()

// ---------- words ----------

internal fun times(n: Int) = if (n == 1) "once" else if (n == 2) "twice" else "$n times"

internal fun songs(n: Int) = if (n == 1) "1 song" else "$n songs"

// ---------- episodes ----------

/** A little of the start does not count, nor does the end: a played episode reads as played. */
internal const val EPISODE_DONE_AT = 0.95f

internal fun episodeFraction(positionMs: Long, durationMs: Long): Float =
    if (durationMs <= 0 || positionMs <= 0) 0f else (positionMs.toFloat() / durationMs).coerceIn(0f, 1f)

/** Where an episode picks up: where it was left, unless that was hardly begun or as good as finished. */
internal fun episodeResumeAt(savedMs: Long, durationMs: Long): Long =
    if (savedMs < EPISODE_RESUMES_FROM_MS || episodeFraction(savedMs, durationMs) >= EPISODE_DONE_AT) 0 else savedMs

private const val EPISODE_RESUMES_FROM_MS = 30_000L

/** The line under an episode: how long, how much is left, or that it is played. */
internal fun episodeLine(positionMs: Long, durationMs: Long, fallback: String): String {
    if (durationMs <= 0) return fallback
    val fraction = episodeFraction(positionMs, durationMs)
    if (fraction >= EPISODE_DONE_AT) return "Played"
    val minutes = { ms: Long -> ((ms + 59_999) / 60_000).let { if (it >= 60) "${it / 60} hr ${it % 60} min" else "$it min" } }
    return if (positionMs > 0) "${minutes(durationMs - positionMs)} left" else minutes(durationMs)
}

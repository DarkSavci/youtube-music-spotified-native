package dev.darksavci.spotified.playback

import android.content.ComponentName
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.session.LibraryResult
import androidx.media3.session.MediaBrowser
import androidx.media3.session.SessionToken
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.darksavci.spotified.graph
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.guava.await
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * What Android Auto asks of the app, asked the way it asks: a media browser
 * walks the tree, searches, and plays a song chosen in a list. Runs against
 * the real core, so it needs the network; signed in, the library is there.
 */
@RunWith(AndroidJUnit4::class)
class BrowseTreeTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private lateinit var browser: MediaBrowser

    @Before
    fun connect() = runBlocking {
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        browser = withContext(Dispatchers.Main) { MediaBrowser.Builder(context, token).buildAsync().await() }
    }

    @After
    fun release() = runBlocking { withContext(Dispatchers.Main) { browser.release() } }

    private suspend fun children(id: String): List<MediaItem> = withContext(Dispatchers.Main) {
        val result = browser.getChildren(id, 0, 200, null).await()
        assertEquals("children of $id", LibraryResult.RESULT_SUCCESS, result.resultCode)
        result.value.orEmpty()
    }

    @Test
    fun theRootHasHomeLibraryLikedAndRecent() = runBlocking {
        val root = withContext(Dispatchers.Main) { browser.getLibraryRoot(null).await() }.value!!
        val tabs = children(root.mediaId).map { it.mediaMetadata.title.toString() }
        assertEquals(listOf("Home", "Library", "Liked", "Recent"), tabs)
    }

    @Test
    fun homeShelvesOpenToSomethingToPlay() = runBlocking {
        val shelves = children(BrowseTree.HOME)
        assertTrue("Home has shelves", shelves.isNotEmpty())
        val inside = children(shelves.first().mediaId)
        assertTrue("a shelf has items", inside.isNotEmpty())
        // Artwork reaches the car through the app's provider, as content://.
        val art = inside.firstNotNullOfOrNull { it.mediaMetadata.artworkUri }
        assertTrue("artwork is content://: $art", art == null || art.scheme == "content")
    }

    @Test
    fun theLibraryOpensToSongs() = runBlocking {
        val library = children(BrowseTree.LIBRARY)
        assertTrue("the library has items", library.isNotEmpty())
        val songs = children(library.first().mediaId)
        assertTrue("a library item has songs", songs.isNotEmpty())
        assertTrue("songs are playable", songs.all { it.mediaMetadata.isPlayable == true })
    }

    @Test
    fun aSongChosenInAListPlaysTheListFromIt() = runBlocking {
        val recent = children(BrowseTree.RECENT)
        assertTrue("there is something recent", recent.size >= 3)
        val chosen = recent[2]
        withContext(Dispatchers.Main) {
            browser.setMediaItem(chosen)
            browser.prepare()
            browser.volume = 0f
        }
        val playback = context.graph.playback
        withTimeout(30_000) {
            while (playback.projection.value.state.current?.title != chosen.mediaMetadata.title.toString()) delay(200)
        }
        val queue = playback.projection.value.state.queue
        assertEquals("the whole list is queued", recent.size, queue.items.size)
        assertEquals("from the chosen song", 2, queue.index)
    }

    @Test
    fun searchFindsSongs() = runBlocking {
        val found = withContext(Dispatchers.Main) {
            browser.search("adamlar", null).await()
            withTimeout(30_000) {
                var items: List<MediaItem> = emptyList()
                while (items.isEmpty()) {
                    items = browser.getSearchResult("adamlar", 0, 20, null).await().value.orEmpty()
                    if (items.isEmpty()) delay(300)
                }
                items
            }
        }
        assertTrue("songs found", found.all { it.mediaMetadata.mediaType == MediaMetadata.MEDIA_TYPE_MUSIC })
    }
}

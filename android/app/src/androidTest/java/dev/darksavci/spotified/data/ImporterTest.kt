package dev.darksavci.spotified.data

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.darksavci.spotified.graph
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * A public playlist from each service the importer reads, read from the
 * service itself and matched against the real core. Needs the network, and
 * the playlists to stay public.
 */
@RunWith(AndroidJUnit4::class)
class ImporterTest {
    private val graph = InstrumentationRegistry.getInstrumentation().targetContext.graph

    private fun imports(link: String) = runBlocking {
        val result = withTimeout(180_000) { Importer.match(graph.http, graph.api, link) }
        println("Imported ${result.name}: ${result.wanted} wanted, ${result.found.size} found, ${result.doubtful.size} doubtful, ${result.missing.size} missing")
        assertTrue("nothing wanted from $link", result.wanted >= 10)
        // Most songs of a chart playlist are on YouTube Music.
        assertTrue("only ${result.found.size} of ${result.wanted} found from $link", result.found.size * 10 >= result.wanted * 8)
    }

    @Test
    fun deezer() = imports("https://www.deezer.com/playlist/3155776842")

    @Test
    fun spotify() = imports("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M")

    @Test
    fun appleMusic() = imports("https://music.apple.com/us/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb")

    @Test
    fun pastedLines() = imports((1..10).joinToString("\n") { listOf("Queen - Bohemian Rhapsody", "The Weeknd - Blinding Lights")[it % 2] })
}

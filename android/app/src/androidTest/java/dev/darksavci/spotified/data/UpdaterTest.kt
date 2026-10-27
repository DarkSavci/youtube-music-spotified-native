package dev.darksavci.spotified.data

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.darksavci.spotified.graph
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The updater against the project's real, published releases. Their newest
 * carries the desktop's files only, so an older phone app is told there is
 * nothing for it, and a newer one that it is up to date.
 */
@RunWith(AndroidJUnit4::class)
class UpdaterTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    private fun said(current: String): List<String> = runBlocking {
        val said = mutableListOf<String>()
        Updater.check(context, context.graph.http, { said += it }, current)
        println("Updater at $current said: $said")
        said
    }

    @Test
    fun anOlderAppIsToldTheNewestReleaseHasNoApk() {
        val said = said("0.0.1")
        assertEquals(1, said.size)
        assertTrue(said[0], Regex("""Version [0-9.]+ has no APK for this phone\.""").matches(said[0]))
    }

    @Test
    fun aNewerAppIsUpToDate() {
        assertEquals(listOf("This is the newest version (99.0.0)."), said("99.0.0"))
    }
}

package dev.darksavci.spotified.data

import java.nio.file.Files
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LogsTest {
    private val dir = Files.createTempDirectory("logs").toFile()

    @Test
    fun aSessionNeverReachesTheFile() {
        val scrubbed = Files.createTempDirectory("scrubbed").toFile()
        Logs.startFile(scrubbed)
        Logs.fromCore(4, "fetch https://rr1.googlevideo.com/videoplayback?sig=abc failed for me@example.com")
        Logs.w("Test", "Cookie: SAPISID=secret", RuntimeException("no"))
        val written = runBlocking { Logs.text() }
        assertFalse(written, written.contains("sig=abc"))
        assertFalse(written, written.contains("me@example.com"))
        assertFalse(written, written.contains("secret"))
        assertTrue(written, written.contains("<redacted>"))
        assertTrue(written, written.contains("<email>"))
    }

    @Test
    fun linesComeBackInTheOrderTheyWereWrittenAcrossRotations() {
        val log = RotatingLog(dir, maxBytes = 20, keep = 2)
        (1..12).forEach { log.append("line $it") }
        val lines = log.readAll().trim().lines()
        assertEquals("line 12", lines.last())
        assertEquals(lines.map { it.removePrefix("line ").toInt() }.sorted(), lines.map { it.removePrefix("line ").toInt() })
    }

    @Test
    fun itNeverKeepsMoreThanItsFiles() {
        val log = RotatingLog(dir, maxBytes = 20, keep = 2)
        (1..100).forEach { log.append("line $it") }
        assertEquals(setOf("app.log", "app.log.1", "app.log.2"), dir.list()!!.toSet())
        assertFalse(log.readAll().contains("line 1\n"))
        assertTrue(dir.listFiles()!!.all { it.length() <= 20 })
    }

    @Test
    fun aRestartCarriesOnFromTheSameFile() {
        RotatingLog(dir, maxBytes = 1000).append("before")
        val again = RotatingLog(dir, maxBytes = 1000)
        again.append("after")
        assertEquals("before\nafter\n", again.readAll())
    }

    @Test
    fun aLongLineIsCut() {
        val log = RotatingLog(dir, maxBytes = 1000, maxLineChars = 10)
        log.append("x".repeat(100))
        assertEquals("x".repeat(10) + "\n", log.readAll())
    }
}

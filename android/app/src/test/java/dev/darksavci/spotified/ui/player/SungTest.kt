package dev.darksavci.spotified.ui.player

import androidx.compose.ui.graphics.Color
import dev.darksavci.spotified.api.LyricLine
import dev.darksavci.spotified.api.LyricWord
import dev.darksavci.spotified.api.Lyrics
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SungTest {
    private val dim = Color.White.copy(alpha = 0.45f)
    private val line = LyricLine(
        atMs = 1000, text = "Hello there world",
        words = listOf(LyricWord(1000, "Hello "), LyricWord(1500, "there "), LyricWord(2100, "world")),
    )

    /** The dimmed part of a line: the words not yet sung. */
    private fun dimmed(atMs: Long): String {
        val text = sung(line, atMs, dim)
        return text.spanStyles.filter { it.item.color == dim }.joinToString("") { text.text.substring(it.start, it.end) }
    }

    @Test
    fun wordsLightAsTheyAreSung() {
        assertEquals("Hello there world", sung(line, 0, dim).text)
        assertEquals("Hello there world", dimmed(999))
        assertEquals("there world", dimmed(1000))
        assertEquals("world", dimmed(2000))
        assertEquals("", dimmed(2100))
    }

    @Test
    fun aLineWithoutWordsIsPlain() {
        val plain = sung(LyricLine(1000, "Just a line"), 0, dim)
        assertEquals("Just a line", plain.text)
        assertTrue(plain.spanStyles.isEmpty())
        assertEquals("♪", sung(LyricLine(1000, ""), 0, dim).text)
    }

    @Test
    fun theCoreSendsWordsWithTheLine() {
        val json = Json { ignoreUnknownKeys = true }
        val lyrics = json.decodeFromString<Lyrics>(
            """{"synced":true,"lines":[{"atMs":1000,"text":"Hi you","words":[{"atMs":1000,"text":"Hi "},{"atMs":1400,"text":"you"}]},{"atMs":3000,"text":"Plain"}]}""",
        )
        assertEquals(listOf(LyricWord(1000, "Hi "), LyricWord(1400, "you")), lyrics.lines[0].words)
        assertTrue(lyrics.lines[1].words.isEmpty())
    }
}

package dev.darksavci.spotified.ui.player

import androidx.compose.ui.graphics.Color
import dev.darksavci.spotified.api.LyricLine
import dev.darksavci.spotified.api.LyricWord
import dev.darksavci.spotified.api.Lyrics
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SungTest {
    private val others = Color.White.copy(alpha = 0.78f)
    private val line = LyricLine(
        atMs = 1000, text = "Hello there world",
        words = listOf(LyricWord(1000, "Hello "), LyricWord(1500, "there "), LyricWord(2100, "world")),
    )

    /** What of a line is at the text's own colour, which is the word being sung. */
    private fun lit(line: LyricLine, atMs: Long): String {
        val text = sung(line, atMs, others)
        val away = text.spanStyles.filter { it.item.color == others }.map { it.start until it.end }
        return text.text.filterIndexed { i, _ -> away.none { i in it } }
    }

    @Test
    fun onlyTheWordBeingSungIsLit() {
        assertEquals("Hello there world", sung(line, 0, others).text)
        assertEquals("", lit(line, 999))
        assertEquals("Hello ", lit(line, 1000))
        assertEquals("there ", lit(line, 2000))
        assertEquals("world", lit(line, 2100))
        // Past the last word, it stays lit: nothing says when a word ends.
        assertEquals("world", lit(line, 9000))
    }

    @Test
    fun aSpaceTimedAsAWordIsNeverTheOneBeingSung() {
        val spaced = LyricLine(1000, "Hi you", words = listOf(LyricWord(1000, "Hi"), LyricWord(1200, " "), LyricWord(1400, "you")))
        assertEquals("Hi", lit(spaced, 1300))
        assertEquals("you", lit(spaced, 1400))
    }

    @Test
    fun onlyATimedLineWithWordsIsLit() {
        val lyrics = Lyrics(synced = true, lines = listOf(LyricLine(1000, "One"), LyricLine(3000, ""), LyricLine(5000, "Two")))
        assertNull(lyrics.lineToLight(500))
        assertEquals(0, lyrics.lineToLight(1000))
        // The source's mark for an instrumental gap.
        assertNull(lyrics.lineToLight(3500))
        assertEquals(2, lyrics.lineToLight(5000))
        assertNull(lyrics.copy(synced = false).lineToLight(5000))
    }

    @Test
    fun aLineWithoutWordsIsPlain() {
        val plain = sung(LyricLine(1000, "Just a line"), 0, others)
        assertEquals("Just a line", plain.text)
        assertTrue(plain.spanStyles.isEmpty())
        assertEquals("♪", sung(LyricLine(1000, ""), 0, others).text)
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

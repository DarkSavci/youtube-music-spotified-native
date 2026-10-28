package dev.darksavci.spotified.car

import androidx.car.app.CarContext
import androidx.car.app.model.Action
import androidx.car.app.model.Header
import androidx.car.app.model.Row
import androidx.car.app.model.RowSection
import androidx.car.app.model.SectionedItemTemplate
import androidx.car.app.model.Template
import androidx.lifecycle.lifecycleScope
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Lyrics
import dev.darksavci.spotified.api.Track
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * The song's lyrics, following it: the line being sung, marked, between
 * the one before and the next few. Lyrics without timings are shown whole.
 * The screen is drawn again only when the line changes.
 */
class LyricsScreen(carContext: CarContext) : Page(carContext) {
    private var track: Track? = null
    private var lyrics: Lyrics? = null
    private var loadingLyrics = true
    private var line = -1
    private var loader: Job? = null

    init {
        lifecycleScope.launch {
            while (true) {
                follow()
                delay(TICK_MS)
            }
        }
    }

    /** Loads lyrics when the song changes, and redraws when the sung line does. */
    private suspend fun follow() {
        val current = graph.playback.projection.value.state.current
        if (current?.id != track?.id) {
            track = current
            lyrics = null
            line = -1
            loadingLyrics = current != null
            loader?.cancel()
            invalidate()
            if (current != null) {
                loader = lifecycleScope.launch {
                    lyrics = runCatching { graph.api.lyrics(current) }.getOrNull()?.withLines()
                    loadingLyrics = false
                    invalidate()
                }
            }
            return
        }
        val synced = lyrics?.takeIf { it.synced } ?: return
        current ?: return
        val engine = graph.playback.engine ?: return
        if (engine.currentId != current?.id) return
        val offset = graph.settings.lyricsOffsets.first()[current.id] ?: 0L
        val now = synced.lineAt(engine.positionMs + offset) ?: 0
        if (now != line) {
            line = now
            invalidate()
        }
    }

    override fun onGetTemplate(): Template {
        val template = SectionedItemTemplate.Builder()
            .setHeader(Header.Builder().setStartHeaderAction(Action.BACK).setTitle(track?.title ?: "Lyrics").build())
        val lines = lyrics?.lines.orEmpty()
        when {
            track == null -> return message(template, "Nothing is playing.")
            loadingLyrics -> return template.setLoading(true).build()
            lines.isEmpty() -> return message(template, "No lyrics for this song.")
        }
        val synced = lyrics?.synced == true
        // Timed lyrics: the line before, the sung one, and what comes; untimed: from the top.
        val from = if (synced) (line - 1).coerceAtLeast(0) else 0
        val shown = lines.withIndex().drop(from).take(if (synced) WINDOW else limit())
        template.addSection(
            RowSection.Builder()
                .setTitle(listOf(track?.artistNames.orEmpty(), lyrics?.source.orEmpty()).filter { it.isNotEmpty() }.joinToString(" • "))
                .setItems(
                    shown.map { (i, l) ->
                        Row.Builder()
                            .setTitle(l.text.ifEmpty { "♪" })
                            .apply { if (synced && i == line) setImage(icon(R.drawable.ic_mic_vocal)) }
                            .build()
                    },
                )
                .build(),
        )
        template.addAction(nowPlayingAction())
        return template.build()
    }

    private fun message(template: SectionedItemTemplate.Builder, text: String): Template =
        template.addSection(RowSection.Builder().setNoItemsMessage(text).build()).build()

    companion object {
        private const val TICK_MS = 250L
        private const val WINDOW = 6
    }
}

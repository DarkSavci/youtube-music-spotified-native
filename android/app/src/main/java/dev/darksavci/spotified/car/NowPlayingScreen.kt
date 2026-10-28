package dev.darksavci.spotified.car

import androidx.annotation.DrawableRes
import androidx.car.app.CarContext
import androidx.car.app.media.model.MediaPlaybackTemplate
import androidx.car.app.model.Action
import androidx.car.app.model.Header
import androidx.car.app.model.Row
import androidx.car.app.model.RowSection
import androidx.car.app.model.SectionedItemTemplate
import androidx.car.app.model.Template
import androidx.lifecycle.lifecycleScope
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Repeat
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

/**
 * What plays, drawn by the car from the session, with the queue and the
 * lyrics in its header: the car allows two buttons there.
 */
class NowPlayingScreen(carContext: CarContext) : Page(carContext) {
    override fun onGetTemplate(): Template {
        val current = graph.playback.projection.value.state.current
        val header = Header.Builder()
            .setStartHeaderAction(Action.BACK)
            .addEndHeaderAction(
                Action.Builder()
                    .setIcon(icon(R.drawable.ic_list_music))
                    .setOnClickListener { screenManager.push(QueueScreen(carContext)) }
                    .build(),
            )
        if (current != null) {
            header.addEndHeaderAction(
                Action.Builder()
                    .setIcon(icon(R.drawable.ic_mic_vocal))
                    .setOnClickListener { screenManager.push(LyricsScreen(carContext)) }
                    .build(),
            )
        }
        return MediaPlaybackTemplate.Builder().setHeader(header.build()).build()
    }
}

/**
 * The queue: what plays now and what comes next. A song jumps there; its
 * button takes it out. Shuffle and repeat sit at the bottom.
 */
class QueueScreen(carContext: CarContext) : Page(carContext) {
    init {
        lifecycleScope.launch {
            graph.playback.projection
                .map { it.state.let { s -> listOf(s.queue.items.map { t -> t.id }, s.queue.index, s.shuffle, s.repeat) } }
                .distinctUntilChanged()
                .collect { invalidate() }
        }
    }

    override fun onGetTemplate(): Template {
        val state = graph.playback.projection.value.state
        val queue = state.queue
        val template = SectionedItemTemplate.Builder()
            .setHeader(Header.Builder().setStartHeaderAction(Action.BACK).setTitle("Queue").build())
        val current = state.current
        if (current == null) {
            return template.addSection(RowSection.Builder().setNoItemsMessage("Nothing is playing.").build()).build()
        }
        template.addSection(
            RowSection.Builder()
                .setTitle("Now playing")
                .addItem(
                    Row.Builder()
                        .setTitle(current.title)
                        .apply { if (current.artistNames.isNotEmpty()) addText(current.artistNames) }
                        .apply { art(current.artwork, 120)?.let { setImage(it, Row.IMAGE_TYPE_LARGE) } }
                        // A radio from what plays: its button.
                        .addAction(Action.Builder().setIcon(icon(R.drawable.ic_radio)).setOnClickListener { graph.playback.startRadio(current) }.build())
                        .setOnClickListener { showNowPlaying(screenManager, carContext) }
                        .build(),
                )
                .build(),
        )
        val upcoming = (queue.index + 1 until queue.items.size).take(limit() - 1)
        template.addSection(
            RowSection.Builder()
                .setTitle(if (queue.origin.isNotEmpty()) "Next from ${queue.origin}" else "Next up")
                .setNoItemsMessage("Nothing comes next.")
                .setItems(
                    upcoming.map { i ->
                        val track = queue.items[i]
                        Row.Builder()
                            .setTitle(track.title)
                            .apply { if (track.artistNames.isNotEmpty()) addText(track.artistNames) }
                            .apply { art(track.artwork, 120)?.let { setImage(it, Row.IMAGE_TYPE_LARGE) } }
                            // The car allows one button a row: the song's moves.
                            .addAction(
                                Action.Builder()
                                    .setIcon(icon(R.drawable.ic_ellipsis))
                                    .setOnClickListener { screenManager.push(QueueSongScreen(carContext, track.id)) }
                                    .build(),
                            )
                            .setOnClickListener { graph.playback.jump(i) }
                            .build()
                    },
                )
                .build(),
        )
        // Lit while on, as the phone's player shows them.
        template.addAction(fab(R.drawable.ic_shuffle, primary = state.shuffle) { graph.playback.setShuffle(!state.shuffle) })
        val repeatIcon = if (state.repeat == Repeat.One) R.drawable.ic_repeat_1 else R.drawable.ic_repeat
        template.addAction(fab(repeatIcon, primary = state.repeat != Repeat.Off) { graph.playback.cycleRepeat() })
        return template.build()
    }
}

/**
 * One queued song's moves: play it now or next, up or down a place (the
 * screen stays, to move it again), to the end, a radio from it, or out of the queue.
 */
class QueueSongScreen(carContext: CarContext, private val id: String) : Page(carContext) {
    init {
        lifecycleScope.launch {
            graph.playback.projection
                .map { it.state.queue.let { q -> q.items.map { t -> t.id } to q.index } }
                .distinctUntilChanged()
                .collect { invalidate() }
        }
    }

    override fun onGetTemplate(): Template {
        val queue = graph.playback.projection.value.state.queue
        val next = queue.index + 1
        // The song is found by its id: the place it was at moves with every change.
        val at = (next until queue.items.size).firstOrNull { queue.items[it].id == id }
        val track = at?.let { queue.items[it] }
        val template = SectionedItemTemplate.Builder()
            .setHeader(Header.Builder().setStartHeaderAction(Action.BACK).setTitle(track?.title ?: "Queue").build())
        if (at == null || track == null) {
            return template.addSection(RowSection.Builder().setNoItemsMessage("This song is no longer coming up.").build()).build()
        }
        val last = queue.items.size - 1
        fun move(@DrawableRes res: Int, title: String, enabled: Boolean, stay: Boolean, action: () -> Unit) =
            Row.Builder()
                .setTitle(title)
                .setImage(icon(res))
                .setEnabled(enabled)
                .setOnClickListener {
                    action()
                    if (!stay) finish()
                }
                .build()
        template.addSection(
            RowSection.Builder()
                .setTitle("${at - queue.index} of ${last - queue.index} up next \u2022 ${track.artistNames}")
                .setItems(
                    listOf(
                        move(R.drawable.ic_play, "Play now", true, false) { graph.playback.jump(at) },
                        move(R.drawable.ic_list_plus, "Play next", at > next, false) { graph.playback.move(at, next) },
                        move(R.drawable.ic_chevron_up, "Move up", at > next, true) { graph.playback.move(at, at - 1) },
                        move(R.drawable.ic_chevron_down, "Move down", at < last, true) { graph.playback.move(at, at + 1) },
                        move(R.drawable.ic_list_end, "Move to the end", at < last, false) { graph.playback.move(at, last) },
                        move(R.drawable.ic_radio, "Start radio from this song", true, false) { graph.playback.startRadio(track) },
                        move(R.drawable.ic_trash_2, "Remove from queue", true, false) { graph.playback.remove(at) },
                    ),
                )
                .build(),
        )
        return template.build()
    }
}

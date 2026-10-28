package dev.darksavci.spotified.car

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
 * What plays, drawn by the car from the session, with the queue and a radio
 * from the song in its header.
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
                    .setIcon(icon(R.drawable.ic_radio))
                    .setOnClickListener { graph.playback.startRadio(current) }
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
                            .addAction(
                                Action.Builder()
                                    .setIcon(icon(R.drawable.ic_x))
                                    .setOnClickListener { graph.playback.remove(i) }
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

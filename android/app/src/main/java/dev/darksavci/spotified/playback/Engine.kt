package dev.darksavci.spotified.playback

import android.content.Context
import android.media.AudioManager
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import dev.darksavci.spotified.api.EngineEvent
import dev.darksavci.spotified.api.EngineEventKind
import dev.darksavci.spotified.api.Target
import kotlin.math.min
import kotlin.math.pow
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

/**
 * The audio engine: two ExoPlayer decks reconciling toward the core's
 * [Target], as crates/audio does on the desktop. One deck plays; the other
 * holds the next track ready, so a track runs into the next without a gap,
 * or fades into it. Everything here runs on the main looper.
 *
 * It reports to the core through [report], in order: loaded, position (once
 * a second while playing), ended, failed, stalled. Position reports are load
 * bearing: the core builds its play log and its "ended too early" guard from
 * them.
 */
@OptIn(UnstableApi::class)
class Engine(
    context: Context,
    http: OkHttpClient,
    /** The core's stream address for a track; the idle deck's carries ?preload=1. */
    private val streamUri: (id: String, preload: Boolean) -> Uri,
    /** A track's loudness in LKFS, fetched off the main thread; null when unknown. */
    private val loudnessOf: suspend (String) -> Double?,
    private val report: (EngineEvent) -> Unit,
    /** Asks why a track failed: true when YouTube is rate limiting, not the track at fault. */
    private val rateLimited: suspend (String) -> Boolean,
    private val scope: CoroutineScope,
) {
    private val main = Handler(Looper.getMainLooper())
    private val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager

    /** One audio session for both decks, so one equalizer acts on whichever plays. */
    val audioSessionId: Int = audioManager.generateAudioSessionId()

    // The core's streams over HTTP, and a music file on the phone by its content:// address.
    private val dataSource = DefaultDataSource.Factory(context, OkHttpDataSource.Factory(http))
    private val decks = arrayOf(newDeck(context), newDeck(context))
    private var playing = 0
    private val current get() = decks[playing]
    private val idle get() = decks[1 - playing]

    private var target = Target()
    private var volume = 1f
    private var duck = 1f
    private var normalise = true
    private var speed = 1f
    private var skipSilence = false

    /** A fade in progress into the idle deck, as the time it started. */
    private var fadeStartedAt = 0L
    private var fadeMs = 0L

    private val listeners = mutableListOf<() -> Unit>()

    /** The track on the playing deck, its position and length, for the screens. */
    val currentId: String? get() = current.id
    val positionMs: Long get() = current.player.currentPosition.coerceAtLeast(0)
    val durationMs: Long get() = current.player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: 0
    val isPlaying: Boolean get() = current.player.isPlaying
    val isBuffering: Boolean get() = current.player.playbackState == Player.STATE_BUFFERING

    fun onChange(listener: () -> Unit) {
        listeners += listener
    }

    private fun changed() = listeners.forEach { it() }

    private inner class Deck(val player: ExoPlayer) {
        var id: String? = null
        var epoch: Long = 0
        var loadedReported = false
        var ended = false
        var gain = 1f
        /** When the deck began waiting for audio while meant to play; 0 when it is not. */
        var stalledSince = 0L
        var failedEpoch = -1L
    }

    private fun newDeck(context: Context): Deck {
        // A few seconds ahead is plenty from a local relay, and keeps memory down.
        val load = DefaultLoadControl.Builder().setBufferDurationsMs(15_000, 60_000, 1_000, 2_000).build()
        val player = ExoPlayer.Builder(context)
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSource))
            .setLoadControl(load)
            // Focus is the service's to manage: two decks must not fight over it.
            .setAudioAttributes(AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MUSIC).build(), false)
            .setHandleAudioBecomingNoisy(false)
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .build()
        player.audioSessionId = audioSessionId
        val deck = Deck(player)
        player.addListener(object : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) = onState(deck, state)
            override fun onPlayerError(error: PlaybackException) = onError(deck, error)
            override fun onIsPlayingChanged(isPlaying: Boolean) = changed()
        })
        return deck
    }

    // ---------- the target ----------

    /** Moves the decks toward [next], the target the core sent. */
    fun apply(next: Target) {
        val previous = target
        target = next
        val decksNow = Decks(current.id, positionMs, current.ended, idle.id)
        for (op in reconcile(decksNow, previous, next)) {
            when (op) {
                Op.Stop -> { stop(current); stop(idle) }
                is Op.Load -> {
                    cancelFade()
                    load(current, next.videoId, op.startMs, preload = false)
                }
                Op.Promote -> {
                    cancelFade()
                    stop(current)
                    playing = 1 - playing
                    current.loadedReported = false
                }
                is Op.Seek -> {
                    current.ended = false
                    current.player.seekTo(op.positionMs)
                }
                Op.Preload -> load(idle, next.preloadVideoId, 0, preload = true)
                Op.DropPreload -> if (fadeStartedAt == 0L) stop(idle)
            }
        }
        // The playing deck acts for this target from now on, including one it
        // ran into by itself before the core named it.
        if (current.id == next.videoId && current.epoch != next.epoch) {
            current.epoch = next.epoch
            current.loadedReported = false
            if (current.player.playbackState == Player.STATE_READY) reportLoaded(current)
        }
        current.player.playWhenReady = next.playing
        if (!next.playing) cancelFade()
        volume = next.volume
        applyVolumes()
        tick()
        changed()
    }

    private fun load(deck: Deck, id: String, startMs: Long, preload: Boolean) {
        deck.id = id
        deck.epoch = if (preload) 0 else target.epoch
        deck.loadedReported = false
        deck.ended = false
        deck.stalledSince = 0
        deck.gain = 1f
        deck.player.setMediaItem(MediaItem.Builder().setMediaId(id).setUri(streamUri(id, preload)).build(), startMs)
        deck.player.playWhenReady = false
        deck.player.prepare()
        if (normalise) scope.launchMain { deck.gain = gainOf(loudnessOf(id)); applyVolumes() }
    }

    private fun stop(deck: Deck) {
        deck.player.stop()
        deck.player.clearMediaItems()
        deck.id = null
        deck.epoch = 0
        deck.ended = false
        deck.stalledSince = 0
    }

    // ---------- what the decks say ----------

    private fun onState(deck: Deck, state: Int) {
        if (deck === current) {
            when (state) {
                Player.STATE_READY -> {
                    deck.stalledSince = 0
                    reportLoaded(deck)
                }
                Player.STATE_BUFFERING ->
                    if (deck.player.playWhenReady && deck.stalledSince == 0L) deck.stalledSince = now()
                Player.STATE_ENDED -> onEnded(deck)
            }
        }
        changed()
    }

    private fun reportLoaded(deck: Deck) {
        if (deck.loadedReported || deck.epoch == 0L) return
        deck.loadedReported = true
        report(EngineEvent(EngineEventKind.Loaded, deck.epoch, durationMs = durationMs))
    }

    /**
     * The playing deck ran out. With the next track ready and a transition
     * that runs on, it starts now and the core is told after; otherwise the
     * core is told and names what comes next.
     */
    private fun onEnded(deck: Deck) {
        deck.ended = true
        val epoch = deck.epoch
        if (target.transition.runsOn && idle.id != null && idle.id == target.preloadVideoId && target.playing) {
            promoteIdle()
        }
        if (epoch != 0L) report(EngineEvent(EngineEventKind.Ended, epoch, positionMs = deck.player.currentPosition))
    }

    /** The idle deck becomes the playing one and starts; the old one is let go. */
    private fun promoteIdle() {
        val old = current
        playing = 1 - playing
        current.loadedReported = false
        current.player.playWhenReady = true
        fadeStartedAt = 0
        stop(old)
        applyVolumes()
    }

    private fun onError(deck: Deck, error: PlaybackException) {
        Log.w(TAG, "deck ${deck.id}: ${error.errorCodeName}", error)
        if (deck !== current) {
            // A failed preload never fails the current track; it is tried
            // again when the core names the track.
            stop(deck)
            return
        }
        val epoch = deck.epoch
        val id = deck.id ?: return
        if (epoch == 0L || deck.failedEpoch == epoch) return
        deck.failedEpoch = epoch
        val network = error.errorCode in PlaybackException.ERROR_CODE_IO_UNSPECIFIED..PlaybackException.ERROR_CODE_IO_NO_PERMISSION
        // Asked before failing a track: a rate limit is waited out, not skipped.
        scope.launchMain {
            val kind = if (rateLimited(id)) EngineEventKind.Blocked else EngineEventKind.Failed
            report(EngineEvent(kind, epoch, positionMs = deck.player.currentPosition, reason = if (network) "network" else ""))
        }
    }

    // ---------- the clock ----------

    private val ticker = object : Runnable {
        override fun run() {
            tick()
            main.postDelayed(this, TICK_MS)
        }
    }
    private var lastPositionReport = 0L

    /** Starts the once-a-second work: position reports, the stall watchdog and fades. */
    fun startClock() {
        main.removeCallbacks(ticker)
        main.post(ticker)
    }

    private fun tick() {
        val deck = current
        val t = now()
        if (deck.player.isPlaying && deck.epoch != 0L && t - lastPositionReport >= REPORT_EVERY_MS) {
            lastPositionReport = t
            report(EngineEvent(EngineEventKind.Position, deck.epoch, positionMs = positionMs, durationMs = durationMs))
        }
        // No audio for a while while meant to play: say so, and read again.
        if (deck.stalledSince != 0L && t - deck.stalledSince > STALL_MS && deck.player.playWhenReady) {
            deck.stalledSince = t
            if (deck.epoch != 0L) report(EngineEvent(EngineEventKind.Stalled, deck.epoch, positionMs = positionMs, reason = "stalled"))
            val at = deck.player.currentPosition
            deck.player.prepare()
            deck.player.seekTo(at)
        }
        crossfade(t)
    }

    /** Starts, runs and ends a fade into the idle deck, at the target's length. */
    private fun crossfade(t: Long) {
        val fade = target.transition.crossfadeMs
        if (fadeStartedAt == 0L) {
            val ready = idle.id != null && idle.id == target.preloadVideoId &&
                idle.player.playbackState == Player.STATE_READY
            if (fade > 0 && ready && current.player.isPlaying && crossfadeDue(positionMs, durationMs, fade, speed)) {
                fadeStartedAt = t
                fadeMs = fade
                idle.player.playWhenReady = true
            }
        } else if (t - fadeStartedAt >= fadeMs) {
            // Faded out: the old track has ended as far as anyone can hear.
            val epoch = current.epoch
            val position = current.player.currentPosition
            promoteIdle()
            if (epoch != 0L) report(EngineEvent(EngineEventKind.Ended, epoch, positionMs = position))
        }
        applyVolumes()
    }

    private fun cancelFade() {
        if (fadeStartedAt == 0L) return
        fadeStartedAt = 0
        idle.player.playWhenReady = false
        idle.player.seekTo(0)
    }

    // ---------- the dials ----------

    fun setDuck(ducked: Boolean) {
        duck = if (ducked) 0.2f else 1f
        applyVolumes()
    }

    fun setNormalise(on: Boolean) {
        normalise = on
        if (!on) decks.forEach { it.gain = 1f }
        applyVolumes()
    }

    fun setSpeed(value: Float) {
        speed = value.coerceIn(0.5f, 3f)
        decks.forEach { it.player.playbackParameters = PlaybackParameters(speed) }
    }

    fun setSkipSilence(on: Boolean) {
        skipSilence = on
        decks.forEach { it.player.skipSilenceEnabled = on }
    }

    /** Each deck's volume: the listener's, the loudness correction, any duck and the fade. */
    private fun applyVolumes() {
        val base = gainFor(volume) * duck
        if (fadeStartedAt == 0L) {
            current.player.volume = base * current.gain
            if (idle.player.isPlaying) idle.player.volume = 0f
            return
        }
        // Equal power: the two together sound as loud as one.
        val p = ((now() - fadeStartedAt).toFloat() / fadeMs).coerceIn(0f, 1f)
        current.player.volume = base * current.gain * kotlin.math.cos(p * Math.PI / 2).toFloat()
        idle.player.volume = base * idle.gain * kotlin.math.sin(p * Math.PI / 2).toFloat()
    }

    fun release() {
        main.removeCallbacks(ticker)
        decks.forEach { it.player.release() }
    }

    private fun now() = SystemClock.elapsedRealtime()

    companion object {
        private const val TAG = "Engine"
        private const val TICK_MS = 100L
        private const val REPORT_EVERY_MS = 1000L
        private const val STALL_MS = 8000L
        /** The loudness tracks are turned down to, as on the desktop. */
        private const val TARGET_LKFS = -14.0

        /** Loud tracks are turned down to the target; quiet ones are never turned up. */
        fun gainOf(lkfs: Double?): Float {
            if (lkfs == null) return 1f
            val db = min(0.0, TARGET_LKFS - lkfs)
            return 10.0.pow(db / 20).toFloat()
        }
    }
}

private fun CoroutineScope.launchMain(block: suspend () -> Unit) {
    launch(Dispatchers.Main) { block() }
}

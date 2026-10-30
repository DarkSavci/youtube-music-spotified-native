package dev.darksavci.spotified.playback

import android.content.Context
import android.media.AudioManager
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.DefaultAudioSink
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import dev.darksavci.spotified.api.EngineEvent
import dev.darksavci.spotified.api.EngineEventKind
import dev.darksavci.spotified.api.Target
import dev.darksavci.spotified.data.Logs
import kotlin.math.cos
import kotlin.math.sin
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
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
    /** Where the core answers now, or null while it is starting. */
    private val coreAddress: () -> String?,
    /** A track's loudness in LKFS, fetched off the main thread; null when unknown. */
    private val loudnessOf: suspend (String) -> Double?,
    private val report: (EngineEvent) -> Unit,
    /** Asks why a track failed: true when YouTube is rate limiting, not the track at fault. */
    private val rateLimited: suspend (String) -> Boolean,
    private val scope: CoroutineScope,
    /** Stretches of a track to skip (SponsorBlock), fetched off the main thread; empty for none. */
    private val skipsOf: suspend (String) -> List<LongRange> = { emptyList() },
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
    /** This phone alone is kept quiet; the decks go on following the target. */
    private var held = false
    private val meantToPlay get() = target.playing && !held
    private var duck = 1f
    private var normalise = true
    private var loudnessTarget = lufsFor("normal")
    private var skipSilence = false

    /** How fast the music plays, 1 being normal. */
    var speed = 1f
        private set

    /** Counts the seeks made on the playing deck, for the media session to notice a jump. */
    var seeks = 0L
        private set

    /** A fade in progress into the idle deck, as the time it started. */
    private var fadeStartedAt = 0L
    private var fadeMs = 0L

    private val listeners = mutableListOf<() -> Unit>()

    private val _spectrum = MutableStateFlow(FloatArray(0))
    /** The sounding deck's bars, each 0 to 1, for as long as something is [watchSpectrum]ing. */
    val spectrum: StateFlow<FloatArray> = _spectrum
    private var spectrumWatched = false

    /** The track on the playing deck, its position and length, for the screens. */
    val currentId: String? get() = current.id
    val positionMs: Long get() = current.player.currentPosition.coerceAtLeast(0)
    val bufferedPositionMs: Long get() = current.player.bufferedPosition.coerceAtLeast(0)
    val durationMs: Long get() = current.player.duration.takeIf { it != C.TIME_UNSET && it > 0 } ?: 0
    val isPlaying: Boolean get() = current.player.isPlaying
    val isBuffering: Boolean get() = current.player.playbackState == Player.STATE_BUFFERING

    fun onChange(listener: () -> Unit) {
        listeners += listener
    }

    private fun changed() = listeners.forEach { it() }

    private inner class Deck(val player: ExoPlayer, val tap: SpectrumTap) {
        var id: String? = null
        var epoch: Long = 0
        var loadedReported = false
        var ended = false
        var gain = 1f
        /** The track's loudness once known, to work the gain out again if the level changes. */
        var lkfs: Double? = null
        /** When the deck began waiting for audio while meant to play; 0 when it is not. */
        var stalledSince = 0L
        var failedEpoch = -1L
        var skips: List<LongRange> = emptyList()
        /** The core's address the track was loaded from; null for a file on the phone. */
        var loadedFrom: String? = null
        /** It has given audio since it was loaded. */
        var hadAudio = false
    }

    private fun moved(deck: Deck) = sourceMoved(deck.loadedFrom, coreAddress())

    private fun newDeck(context: Context): Deck {
        // A few seconds ahead is plenty from a local relay, and keeps memory down.
        val load = DefaultLoadControl.Builder().setBufferDurationsMs(15_000, 60_000, 1_000, 2_000).build()
        val tap = SpectrumTap { _spectrum.value = it }
        val player = ExoPlayer.Builder(context)
            .setRenderersFactory(TappedRenderers(context, tap))
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSource))
            .setLoadControl(load)
            // Focus is the service's to manage: two decks must not fight over it.
            .setAudioAttributes(AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MUSIC).build(), false)
            .setHandleAudioBecomingNoisy(false)
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .build()
        player.audioSessionId = audioSessionId
        val deck = Deck(player, tap)
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
        val wasMeantToPlay = meantToPlay
        target = next
        // After a core restart a deck's address is dead: what it holds counts as nothing.
        val decksNow = Decks(current.id.takeUnless { moved(current) }, positionMs, current.ended, idle.id.takeUnless { moved(idle) })
        val ops = reconcile(decksNow, previous, next)
        // Whatever has to change, the sound moving on to the idle deck no longer
        // follows: a seek back, or repeat one with nothing to move on to.
        if (ops.isNotEmpty()) cancelFade()
        for (op in ops) {
            when (op) {
                Op.Stop -> { stop(current); stop(idle) }
                is Op.Load -> load(current, next.videoId, op.startMs, preload = false)
                Op.Promote -> {
                    stop(current)
                    playing = 1 - playing
                    current.loadedReported = false
                    aimSpectrum()
                }
                is Op.Seek -> {
                    current.ended = false
                    seek(current, op.positionMs, op.reason.label)
                }
                Op.Preload -> load(idle, next.preloadVideoId, 0, preload = true)
                Op.DropPreload -> stop(idle)
            }
        }
        // The playing deck acts for this target from now on, including one it
        // ran into by itself before the core named it.
        if (current.id == next.videoId && current.epoch != next.epoch) {
            current.epoch = next.epoch
            current.loadedReported = false
            if (current.player.playbackState == Player.STATE_READY) reportLoaded(current)
        }
        sound(wasMeantToPlay)
    }

    /** Keeps this phone quiet, or lets it sound again, whatever the target says. */
    fun setHeld(on: Boolean) {
        if (on == held) return
        val wasMeantToPlay = meantToPlay
        held = on
        sound(wasMeantToPlay)
    }

    /** Makes the playing deck sound or not, as is meant now. */
    private fun sound(wasMeantToPlay: Boolean) {
        val deck = current
        deck.player.playWhenReady = meantToPlay
        if (!meantToPlay) cancelFade()
        val idleDeck = deck.player.playbackState == Player.STATE_IDLE
        if (prepareAgain(idleDeck, deck.id != null && deck.id == target.videoId, meantToPlay, wasMeantToPlay, deck.failedEpoch == deck.epoch)) {
            // A failure from here on is a new one, for the core to hear of.
            deck.failedEpoch = -1
            placeAfterFailure(deck.player.currentPosition, target.startAtMs)?.let { seek(deck, it, "prepared again after an error") }
            deck.player.prepare()
        }
        applyVolumes()
        tick()
        changed()
    }

    /** Every seek goes through here, so the log says why each was made. */
    private fun seek(deck: Deck, positionMs: Long, why: String) {
        Logs.w(TAG, "seek ${deck.id} from ${deck.player.currentPosition} to $positionMs ms: $why")
        deck.player.seekTo(positionMs)
        if (deck === current) {
            seeks++
            changed()
        }
    }

    private fun load(deck: Deck, id: String, startMs: Long, preload: Boolean) {
        deck.id = id
        deck.epoch = if (preload) 0 else target.epoch
        deck.loadedReported = false
        deck.ended = false
        deck.stalledSince = 0
        deck.gain = 1f
        deck.lkfs = null
        deck.skips = emptyList()
        deck.failedEpoch = -1
        deck.hadAudio = false
        val uri = streamUri(id, preload)
        deck.loadedFrom = uri.authority.takeIf { uri.scheme == "http" }
        deck.player.setMediaItem(MediaItem.Builder().setMediaId(id).setUri(uri).build(), startMs)
        deck.player.playWhenReady = false
        deck.player.prepare()
        if (normalise) {
            scope.launchMain {
                val lkfs = loudnessOf(id)
                if (deck.id == id) {
                    deck.lkfs = lkfs
                    deck.gain = loudnessGain(lkfs, loudnessTarget)
                    applyVolumes()
                }
            }
        }
        // Checked once they are in: the deck may hold another song by then.
        scope.launchMain {
            val skips = skipsOf(id)
            if (deck.id == id) deck.skips = skips
        }
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
        if (state == Player.STATE_READY) deck.hadAudio = true
        if (deck === current) {
            when (state) {
                Player.STATE_READY -> reportLoaded(deck)
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
        if (target.transition.runsOn && idle.id != null && idle.id == target.preloadVideoId && meantToPlay) {
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
        aimSpectrum()
        applyVolumes()
    }

    private fun onError(deck: Deck, error: PlaybackException) {
        Logs.w(TAG, "deck ${deck.id}: ${error.errorCodeName}", error)
        if (deck !== current) {
            // A failed preload never fails the current track; it is tried
            // again when the core names the track.
            stop(deck)
            return
        }
        val epoch = deck.epoch
        val id = deck.id ?: return
        // The core it was reading from has gone, which says nothing of the song.
        if (moved(deck)) return
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
        // Worked out here, as a deck loads held back and is started while already buffering.
        val waiting = deck.player.playWhenReady && deck.player.playbackState == Player.STATE_BUFFERING
        deck.stalledSince = waitingSince(waiting, deck.stalledSince, t)
        if (stalled(deck.stalledSince, t, deck.hadAudio)) {
            deck.stalledSince = t
            if (deck.epoch != 0L) report(EngineEvent(EngineEventKind.Stalled, deck.epoch, positionMs = positionMs, reason = "stalled"))
            val at = deck.player.currentPosition
            deck.player.prepare()
            seek(deck, at, "stalled, reading again")
        }
        skipMarked(deck)
        crossfade(t)
    }

    /** Past a stretch marked as not the music, when the listener asked for that. */
    private fun skipMarked(deck: Deck) {
        if (deck.skips.isEmpty() || !deck.player.isPlaying) return
        val at = deck.player.currentPosition
        // A second's grace at the end of a stretch, so a seek just short of it does not loop.
        deck.skips.firstOrNull { at in it.first until (it.last - 1000) }?.let { seek(deck, it.last, "past a marked stretch") }
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
        seek(idle, 0, "fade called off, back to the start")
    }

    // ---------- the bars ----------

    /** Starts the bars, or stops them. Stopped, each deck only passes its audio along. */
    fun watchSpectrum(on: Boolean) {
        spectrumWatched = on
        aimSpectrum()
        if (!on) _spectrum.value = FloatArray(0)
    }

    private fun aimSpectrum() {
        decks.forEachIndexed { i, deck -> deck.tap.watching = spectrumWatched && i == playing }
    }

    // ---------- the dials ----------

    fun setDuck(ducked: Boolean) {
        duck = if (ducked) 0.2f else 1f
        applyVolumes()
    }

    fun setNormalise(on: Boolean) {
        normalise = on
        decks.forEach { it.gain = if (on) loudnessGain(it.lkfs, loudnessTarget) else 1f }
        applyVolumes()
    }

    /** The loudness tracks are turned down to, in LUFS. */
    fun setLoudnessTarget(lufs: Double) {
        loudnessTarget = lufs
        if (normalise) decks.forEach { it.gain = loudnessGain(it.lkfs, lufs) }
        applyVolumes()
    }

    fun setSpeed(value: Float) {
        speed = value.coerceIn(0.5f, 3f)
        decks.forEach { it.player.playbackParameters = PlaybackParameters(speed) }
        changed()
    }

    fun setSkipSilence(on: Boolean) {
        skipSilence = on
        decks.forEach { it.player.skipSilenceEnabled = on }
    }

    /**
     * Each deck's volume: the loudness correction, any duck and the fade. The
     * target's volume is not in it: on a phone the system's volume is the
     * volume, and a second dial here only took 10 dB off everything. Nothing on
     * the phone or in a room sets it.
     */
    private fun applyVolumes() {
        val base = duck
        if (fadeStartedAt == 0L) {
            current.player.volume = base * current.gain
            if (idle.player.isPlaying) idle.player.volume = 0f
            return
        }
        // Equal power: the two together sound as loud as one.
        val p = ((now() - fadeStartedAt).toFloat() / fadeMs).coerceIn(0f, 1f)
        current.player.volume = base * current.gain * cos(p * Math.PI / 2).toFloat()
        idle.player.volume = base * idle.gain * sin(p * Math.PI / 2).toFloat()
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
    }
}

/** ExoPlayer's own renderers, but for the tap in front of the sound. */
@OptIn(UnstableApi::class)
private class TappedRenderers(context: Context, private val tap: AudioProcessor) : DefaultRenderersFactory(context) {
    override fun buildAudioSink(context: Context, enableFloatOutput: Boolean, enableAudioOutputPlaybackParams: Boolean): AudioSink =
        DefaultAudioSink.Builder(context)
            .setEnableFloatOutput(enableFloatOutput)
            .setEnableAudioOutputPlaybackParameters(enableAudioOutputPlaybackParams)
            .setAudioProcessors(arrayOf(tap))
            .build()
}

private fun CoroutineScope.launchMain(block: suspend () -> Unit) {
    launch(Dispatchers.Main) { block() }
}

package dev.darksavci.spotified.playback

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.audiofx.Equalizer
import android.net.Uri
import android.os.Bundle
import androidx.annotation.OptIn
import androidx.core.content.ContextCompat
import androidx.media3.common.MediaItem
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.CommandButton
import androidx.media3.session.LibraryResult
import androidx.media3.session.MediaLibraryService
import androidx.media3.session.MediaSession
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionCommands
import androidx.media3.session.SessionResult
import com.google.common.collect.ImmutableList
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.LocalMusic
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.data.SponsorBlock
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.MainActivity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.guava.future
import kotlinx.coroutines.launch

/**
 * Playback, for as long as there is something to play: the engine, the media
 * session the system's surfaces talk to (notification, lock screen,
 * Bluetooth, Android Auto, a watch, the Assistant), audio focus, and the
 * sound settings.
 */
@OptIn(UnstableApi::class)
class PlaybackService : MediaLibraryService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private lateinit var engine: Engine
    private lateinit var player: CorePlayer
    private lateinit var session: MediaLibrarySession
    private lateinit var tree: BrowseTree
    private lateinit var focus: Focus
    private var equalizer: Equalizer? = null
    private var noisyRegistered = false

    override fun onCreate() {
        super.onCreate()
        val g = graph
        engine = Engine(
            context = this,
            http = g.http,
            streamUri = { id, preload ->
                // A file on the phone plays from there; everything else through the core.
                LocalMusic.uriOf(id) ?: Uri.Builder().scheme("http")
                    .encodedAuthority(g.core.currentAddress() ?: "127.0.0.1")
                    .appendPath("v1").appendPath("stream").appendPath(id)
                    .apply { if (preload) appendQueryParameter("preload", "1") }
                    .build()
            },
            loudnessOf = { id -> if (LocalMusic.isLocal(id)) null else runCatching { g.api.loudness(id) }.getOrNull() },
            report = g.playback::report,
            rateLimited = { id -> runCatching { g.api.health(id).rateLimited }.getOrDefault(false) },
            scope = scope,
            skipsOf = { id ->
                if (LocalMusic.isLocal(id) || !g.settings.current().sponsorBlock) emptyList()
                else SponsorBlock.segments(g.http, id)
            },
        )
        player = CorePlayer(g.playback, engine)
        engine.onChange { player.refresh() }
        g.playback.engine = engine
        engine.startClock()
        g.playback.start()
        tree = BrowseTree(g.api, ArtworkProvider::uriFor)
        focus = Focus(this, onLoss = { g.playback.pause() }, onDuck = engine::setDuck, onRegain = { g.playback.resume() })

        scope.launch {
            g.playback.projection.collect { p ->
                player.update(p)
                focus.want(p.target.playing)
                noisy(p.target.playing)
            }
        }
        scope.launch {
            combine(g.playback.projection, g.liked.ids) { p, ids -> p.state.current?.id?.let { it in ids } ?: false }
                .collect { liked -> session.setMediaButtonPreferences(buttons(liked)) }
        }
        scope.launch { g.settings.settings.collect(::applySound) }

        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        session = MediaLibrarySession.Builder(this, player, Callback())
            .setSessionActivity(open)
            .setMediaButtonPreferences(buttons(false))
            .build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaLibrarySession = session

    /** Swiped away while paused: there is nothing to keep the service for. */
    override fun onTaskRemoved(rootIntent: Intent?) {
        if (!graph.playback.projection.value.target.playing) stopSelf()
    }

    override fun onDestroy() {
        graph.playback.engine = null
        session.release()
        player.release()
        engine.release()
        equalizer?.release()
        focus.want(false)
        noisy(false)
        scope.cancel()
        super.onDestroy()
    }

    // ---------- the notification's buttons ----------

    private fun buttons(liked: Boolean): ImmutableList<CommandButton> = ImmutableList.of(
        CommandButton.Builder(if (liked) CommandButton.ICON_HEART_FILLED else CommandButton.ICON_HEART_UNFILLED)
            .setDisplayName(if (liked) "Remove from Liked" else "Like")
            .setSessionCommand(SessionCommand(LIKE, Bundle.EMPTY))
            .setSlots(CommandButton.SLOT_OVERFLOW)
            .build(),
        CommandButton.Builder(CommandButton.ICON_SHUFFLE_ON)
            .setDisplayName("Shuffle")
            .setPlayerCommand(androidx.media3.common.Player.COMMAND_SET_SHUFFLE_MODE)
            .setSlots(CommandButton.SLOT_OVERFLOW)
            .build(),
    )

    // ---------- sound ----------

    private fun applySound(s: Settings) {
        engine.setNormalise(s.normalise)
        engine.setSpeed(s.speed)
        engine.setSkipSilence(s.skipSilence)
        if (s.equalizerOn) {
            val eq = equalizer ?: runCatching { Equalizer(0, engine.audioSessionId) }.getOrNull()?.also { equalizer = it }
            eq?.let { applyBands(it, s.equalizerBands) }
            eq?.enabled = true
        } else {
            equalizer?.enabled = false
        }
    }

    private fun applyBands(eq: Equalizer, bands: String) {
        val levels = bands.split(',').mapNotNull { it.trim().toIntOrNull() }
        val (lo, hi) = eq.bandLevelRange.let { it[0].toInt() to it[1].toInt() }
        for (band in 0 until eq.numberOfBands) {
            eq.setBandLevel(band.toShort(), (levels.getOrNull(band) ?: 0).coerceIn(lo, hi).toShort())
        }
    }

    // ---------- a headset unplugged ----------

    private val noisyReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) graph.playback.pause()
        }
    }

    private fun noisy(listen: Boolean) {
        if (listen == noisyRegistered) return
        noisyRegistered = listen
        if (listen) {
            ContextCompat.registerReceiver(
                this, noisyReceiver, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY), ContextCompat.RECEIVER_NOT_EXPORTED,
            )
        } else {
            runCatching { unregisterReceiver(noisyReceiver) }
        }
    }

    // ---------- the media session ----------

    private inner class Callback : MediaLibrarySession.Callback {
        /**
         * The system, this app, Android Auto, a watch and the Assistant get
         * everything; any other app only the playback controls, not the
         * library to browse or the account's likes.
         */
        override fun onConnect(session: MediaSession, controller: MediaSession.ControllerInfo): MediaSession.ConnectionResult {
            if (!controller.isTrusted && controller.packageName !in KNOWN_HOSTS) {
                return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                    .setAvailableSessionCommands(SessionCommands.EMPTY)
                    .build()
            }
            val commands = MediaSession.ConnectionResult.DEFAULT_SESSION_AND_LIBRARY_COMMANDS.buildUpon()
                .add(SessionCommand(LIKE, Bundle.EMPTY))
                .build()
            return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                .setAvailableSessionCommands(commands)
                .build()
        }

        override fun onCustomCommand(
            session: MediaSession, controller: MediaSession.ControllerInfo, customCommand: SessionCommand, args: Bundle,
        ): ListenableFuture<SessionResult> {
            if (customCommand.customAction == LIKE) {
                graph.playback.projection.value.state.current?.let(graph.liked::toggle)
                return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
            }
            return Futures.immediateFuture(SessionResult(SessionResult.RESULT_ERROR_NOT_SUPPORTED))
        }

        override fun onGetLibraryRoot(
            session: MediaLibrarySession, browser: MediaSession.ControllerInfo, params: LibraryParams?,
        ): ListenableFuture<LibraryResult<MediaItem>> = Futures.immediateFuture(LibraryResult.ofItem(tree.root, params))

        override fun onGetChildren(
            session: MediaLibrarySession, browser: MediaSession.ControllerInfo, parentId: String,
            page: Int, pageSize: Int, params: LibraryParams?,
        ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> = scope.future {
            runCatching { tree.children(parentId) }.fold(
                { all ->
                    val from = (page * pageSize).coerceAtMost(all.size)
                    LibraryResult.ofItemList(all.subList(from, (from + pageSize).coerceAtMost(all.size)), params)
                },
                { LibraryResult.ofError(LibraryResult.RESULT_ERROR_IO) },
            )
        }

        override fun onGetItem(
            session: MediaLibrarySession, browser: MediaSession.ControllerInfo, mediaId: String,
        ): ListenableFuture<LibraryResult<MediaItem>> {
            val found = tree.resolve(mediaId)?.let { (list, i) -> CorePlayer.mediaItemOf(list[i]) }
            return Futures.immediateFuture(
                if (found != null) LibraryResult.ofItem(found, null) else LibraryResult.ofError(LibraryResult.RESULT_ERROR_BAD_VALUE),
            )
        }

        override fun onSearch(
            session: MediaLibrarySession, browser: MediaSession.ControllerInfo, query: String, params: LibraryParams?,
        ): ListenableFuture<LibraryResult<Void>> = scope.future {
            val found = runCatching { tree.search(query) }.getOrDefault(emptyList())
            session.notifySearchResultChanged(browser, query, found.size, params)
            LibraryResult.ofVoid()
        }

        override fun onGetSearchResult(
            session: MediaLibrarySession, browser: MediaSession.ControllerInfo, query: String,
            page: Int, pageSize: Int, params: LibraryParams?,
        ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> = scope.future {
            val all = runCatching { tree.search(query) }.getOrDefault(emptyList())
            val from = (page * pageSize).coerceAtMost(all.size)
            LibraryResult.ofItemList(all.subList(from, (from + pageSize).coerceAtMost(all.size)), params)
        }

        /**
         * What to play when a browser or the Assistant asks: a song chosen in
         * a list plays the list from it; "play X" searches for X.
         */
        override fun onSetMediaItems(
            mediaSession: MediaSession, controller: MediaSession.ControllerInfo,
            mediaItems: MutableList<MediaItem>, startIndex: Int, startPositionMs: Long,
        ): ListenableFuture<MediaSession.MediaItemsWithStartPosition> = scope.future {
            val first = mediaItems.firstOrNull()
            val query = first?.requestMetadata?.searchQuery
            val (tracks, at) = when {
                first == null -> emptyList<Track>() to 0
                !query.isNullOrBlank() -> runCatching { tree.songsFor(query) }.getOrDefault(emptyList()) to 0
                mediaItems.size == 1 && tree.resolve(first.mediaId) != null -> tree.resolve(first.mediaId)!!
                else -> mediaItems.mapNotNull(CorePlayer::trackOf) to startIndex.coerceAtLeast(0)
            }
            MediaSession.MediaItemsWithStartPosition(tracks.map { CorePlayer.mediaItemOf(it) }, at, startPositionMs)
        }

        override fun onAddMediaItems(
            mediaSession: MediaSession, controller: MediaSession.ControllerInfo, mediaItems: MutableList<MediaItem>,
        ): ListenableFuture<MutableList<MediaItem>> = Futures.immediateFuture(
            mediaItems.mapNotNull { item ->
                (tree.resolve(item.mediaId)?.let { (l, i) -> l[i] } ?: CorePlayer.trackOf(item))?.let { CorePlayer.mediaItemOf(it) }
            }.toMutableList(),
        )

        /** After a reboot, the system's media controls offer to carry on: the core kept the queue. */
        override fun onPlaybackResumption(
            mediaSession: MediaSession, controller: MediaSession.ControllerInfo, isForPlayback: Boolean,
        ): ListenableFuture<MediaSession.MediaItemsWithStartPosition> = scope.future {
            graph.core.running()
            val state = graph.playback.projection.value.state
            MediaSession.MediaItemsWithStartPosition(
                state.queue.items.map { CorePlayer.mediaItemOf(it) }, state.queue.index, state.positionMs,
            )
        }
    }

    companion object {
        const val LIKE = "dev.darksavci.spotified.LIKE"

        /** Media hosts that are not the system but may browse: Android Auto, Wear OS, Google's app. */
        private val KNOWN_HOSTS = setOf(
            "com.google.android.projection.gearhead",
            "com.google.android.wearable.app",
            "com.google.android.apps.wearables.maestro.companion",
            "com.google.android.googlequicksearchbox",
            "com.google.android.carassistant",
        )
    }
}

/** Audio focus, held while the core means to play. */
private class Focus(
    context: Context,
    private val onLoss: () -> Unit,
    private val onDuck: (Boolean) -> Unit,
    private val onRegain: () -> Unit,
) : AudioManager.OnAudioFocusChangeListener {
    private val audio = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
    private val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
        .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build())
        .setOnAudioFocusChangeListener(this)
        .setWillPauseWhenDucked(false)
        .build()
    private var held = false
    /** Paused for a call or another app's moment, to carry on after it. */
    private var pausedForTransient = false

    fun want(play: Boolean) {
        if (play && !held) {
            held = audio.requestAudioFocus(request) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
            pausedForTransient = false
        } else if (!play && held && !pausedForTransient) {
            audio.abandonAudioFocusRequest(request)
            held = false
        }
    }

    override fun onAudioFocusChange(change: Int) {
        when (change) {
            AudioManager.AUDIOFOCUS_LOSS -> {
                held = false
                pausedForTransient = false
                audio.abandonAudioFocusRequest(request)
                onLoss()
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> {
                pausedForTransient = true
                onLoss()
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> onDuck(true)
            AudioManager.AUDIOFOCUS_GAIN -> {
                onDuck(false)
                if (pausedForTransient) {
                    pausedForTransient = false
                    onRegain()
                }
            }
        }
    }
}

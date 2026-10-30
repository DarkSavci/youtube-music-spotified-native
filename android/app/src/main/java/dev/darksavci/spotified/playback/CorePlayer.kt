package dev.darksavci.spotified.playback

import android.os.Bundle
import android.os.Looper
import androidx.annotation.OptIn
import androidx.core.net.toUri
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import com.google.common.collect.ImmutableList
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import dev.darksavci.spotified.api.Command
import dev.darksavci.spotified.api.CoreJson
import dev.darksavci.spotified.api.PlayState
import dev.darksavci.spotified.api.Projection
import dev.darksavci.spotified.api.Repeat
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.api.url

/**
 * The core's queue as a Media3 player. The media session, and through it the
 * notification, the lock screen, Bluetooth, Android Auto and a watch, show
 * this; what they ask of it becomes a command to the core. The sound itself
 * comes from [Engine], which follows the core's target.
 */
@OptIn(UnstableApi::class)
class CorePlayer(private val playback: Playback, private val engine: Engine) :
    SimpleBasePlayer(Looper.getMainLooper()) {

    private var projection = Projection()

    /** What the session was last told. */
    private var published: Shown? = null
    private var reportedSeeks = 0L

    /** The queue as the session has it, and the media items made for its tracks. */
    private var playlistOf: List<Track> = emptyList()
    private var playlist: ImmutableList<MediaItemData> = ImmutableList.of()
    private var mediaItems: Map<Track, MediaItem> = emptyMap()

    /**
     * What the system's surfaces can see change. The position is not in it: a
     * projection arrives every second for a playing track, and the surfaces
     * move their seek bars on by themselves from the last position and speed.
     * A jump is a seek, and that is here.
     */
    private data class Shown(
        val items: List<Track>,
        val index: Int,
        val playbackState: Int,
        val playWhenReady: Boolean,
        val shuffle: Boolean,
        val repeat: Repeat,
        val speed: Float,
        val seeks: Long,
    )

    /** Called on the main thread with each projection. */
    fun update(next: Projection) {
        projection = next
        refresh()
    }

    /** Tells the session of anything it should show differently, and of nothing else. */
    fun refresh() {
        val now = shown()
        if (now != published) {
            published = now
            invalidateState()
        }
    }

    private fun shown(): Shown {
        val s = projection.state
        val index = indexIn(s.queue.items)
        return Shown(
            s.queue.items, index, playbackState(s.queue.items.getOrNull(index)), projection.target.playing,
            s.shuffle, s.repeat, engine.speed, engine.seeks,
        )
    }

    private fun indexIn(items: List<Track>) = projection.state.queue.index.coerceIn(0, (items.size - 1).coerceAtLeast(0))

    private fun playbackState(current: Track?): Int {
        val ours = current != null && engine.currentId == current.id
        return when {
            current == null -> Player.STATE_IDLE
            projection.state.state == PlayState.Loading || projection.state.state == PlayState.Stalled -> Player.STATE_BUFFERING
            ours && engine.isBuffering -> Player.STATE_BUFFERING
            ours -> Player.STATE_READY
            projection.target.playing -> Player.STATE_BUFFERING
            else -> Player.STATE_READY
        }
    }

    /** The queue's items, made again only when it has changed, and then only for tracks not seen before. */
    private fun playlistFor(items: List<Track>): ImmutableList<MediaItemData> {
        if (items == playlistOf) return playlist
        val made = HashMap<Track, MediaItem>(items.size)
        val list = ImmutableList.builder<MediaItemData>()
        items.forEachIndexed { i, track ->
            val item = mediaItems[track] ?: mediaItemOf(track)
            made[track] = item
            list.add(itemData(i, track, item))
        }
        playlistOf = items
        mediaItems = made
        playlist = list.build()
        return playlist
    }

    override fun getState(): State {
        val s = projection.state
        val items = s.queue.items
        val index = indexIn(items)
        val current = items.getOrNull(index)
        val playbackState = playbackState(current)
        val ours = { engine.currentId == current?.id }
        val builder = State.Builder()
            .setAvailableCommands(COMMANDS)
            .setPlayWhenReady(projection.target.playing, Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
            .setPlaybackState(playbackState)
            .setShuffleModeEnabled(s.shuffle)
            .setRepeatMode(
                when (s.repeat) {
                    Repeat.Off -> Player.REPEAT_MODE_OFF
                    Repeat.One -> Player.REPEAT_MODE_ONE
                    Repeat.All -> Player.REPEAT_MODE_ALL
                },
            )
            .setAudioAttributes(AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MUSIC).build())
            .setPlaybackParameters(PlaybackParameters(engine.speed))
        if (items.isNotEmpty()) {
            // Read live, as they may be long after this state was made.
            builder.setPlaylist(playlistFor(items))
                .setCurrentMediaItemIndex(index)
                .setContentPositionMs { if (ours()) engine.positionMs else projection.state.positionMs }
                .setContentBufferedPositionMs { if (ours()) engine.bufferedPositionMs else projection.state.positionMs }
                .setTotalBufferedDurationMs { if (ours()) (engine.bufferedPositionMs - engine.positionMs).coerceAtLeast(0) else 0 }
                .setIsLoading(playbackState == Player.STATE_BUFFERING)
            if (engine.seeks != reportedSeeks && ours()) {
                builder.setPositionDiscontinuity(Player.DISCONTINUITY_REASON_SEEK, engine.positionMs)
            }
            reportedSeeks = engine.seeks
        }
        return builder.build()
    }

    private fun itemData(index: Int, track: Track, item: MediaItem): MediaItemData =
        MediaItemData.Builder("$index:${track.id}")
            .setMediaItem(item)
            .setDurationUs(if (track.durationMs > 0) track.durationMs * 1000 else C.TIME_UNSET)
            .build()

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        // Not a toggle: this often follows what handleSetMediaItems just sent, whose answer is not in yet.
        if (playWhenReady) playback.resume() else playback.pressPause()
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        val current = projection.state.queue.index
        when (seekCommand) {
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> playback.next()
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> playback.previous()
            else -> when {
                mediaItemIndex != C.INDEX_UNSET && mediaItemIndex != current -> playback.jump(mediaItemIndex)
                positionMs != C.TIME_UNSET -> playback.seek(positionMs)
            }
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleSetShuffleModeEnabled(shuffleModeEnabled: Boolean): ListenableFuture<*> {
        playback.setShuffle(shuffleModeEnabled)
        return Futures.immediateVoidFuture()
    }

    override fun handleSetRepeatMode(repeatMode: Int): ListenableFuture<*> {
        playback.setRepeat(
            when (repeatMode) {
                Player.REPEAT_MODE_ONE -> Repeat.One
                Player.REPEAT_MODE_ALL -> Repeat.All
                else -> Repeat.Off
            },
        )
        return Futures.immediateVoidFuture()
    }

    override fun handleSetMediaItems(mediaItems: List<MediaItem>, startIndex: Int, startPositionMs: Long): ListenableFuture<*> {
        val tracks = mediaItems.mapNotNull(::trackOf)
        if (tracks.isNotEmpty()) {
            val at = if (startIndex == C.INDEX_UNSET) 0 else startIndex.coerceIn(0, tracks.size - 1)
            val origin = mediaItems.firstOrNull()?.mediaMetadata?.albumTitle?.toString().orEmpty()
            val start = startPositionMs.takeIf { it != C.TIME_UNSET && it > 0 }
            // The newest projection, not this player's: on a cold start it may be a step behind.
            val state = playback.projection.value.state
            when (val what = handed(state.queue.items.map { it.id }, state.queue.index, state.positionMs, tracks.map { it.id }, at, start)) {
                // Nothing is started here: what plays it is the play that follows, if one does.
                is Handed.SameQueue -> {
                    what.jumpTo?.let(playback::jump)
                    what.seekTo?.let(playback::seek)
                }
                Handed.NewQueue ->
                    if (start != null) playback.playFrom(tracks, at, origin, start) else playback.play(tracks, at, origin)
            }
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleAddMediaItems(index: Int, mediaItems: List<MediaItem>): ListenableFuture<*> {
        val tracks = mediaItems.mapNotNull(::trackOf)
        if (tracks.isNotEmpty()) playback.send(Command.Enqueue(tracks, index))
        return Futures.immediateVoidFuture()
    }

    override fun handleMoveMediaItems(fromIndex: Int, toIndex: Int, newIndex: Int): ListenableFuture<*> {
        // The core moves one entry at a time.
        if (toIndex - fromIndex == 1) playback.move(fromIndex, newIndex)
        return Futures.immediateVoidFuture()
    }

    override fun handleRemoveMediaItems(fromIndex: Int, toIndex: Int): ListenableFuture<*> {
        for (i in (toIndex - 1) downTo fromIndex) playback.remove(i)
        return Futures.immediateVoidFuture()
    }

    override fun handleStop(): ListenableFuture<*> {
        playback.pause()
        return Futures.immediateVoidFuture()
    }

    override fun handleRelease(): ListenableFuture<*> = Futures.immediateVoidFuture()

    companion object {
        private val COMMANDS = Player.Commands.Builder().addAll(
            Player.COMMAND_PLAY_PAUSE,
            Player.COMMAND_STOP,
            Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_DEFAULT_POSITION,
            Player.COMMAND_SEEK_TO_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_NEXT,
            Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS,
            Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
            Player.COMMAND_SET_SHUFFLE_MODE,
            Player.COMMAND_SET_REPEAT_MODE,
            Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
            Player.COMMAND_GET_TIMELINE,
            Player.COMMAND_GET_METADATA,
            Player.COMMAND_SET_MEDIA_ITEM,
            Player.COMMAND_CHANGE_MEDIA_ITEMS,
            Player.COMMAND_GET_AUDIO_ATTRIBUTES,
            Player.COMMAND_RELEASE,
        ).build()

        private const val TRACK = "dev.darksavci.spotified.TRACK"

        /** A track as a media item, carrying itself so it can be played from Auto or a widget. */
        fun mediaItemOf(track: Track, browsable: Boolean = false): MediaItem {
            val extras = Bundle().apply { putString(TRACK, CoreJson.encodeToString(Track.serializer(), track)) }
            return MediaItem.Builder()
                .setMediaId(track.id)
                .setMediaMetadata(
                    MediaMetadata.Builder()
                        .setTitle(track.title)
                        .setArtist(track.artistNames)
                        .setAlbumTitle(track.album?.name)
                        // Served by the app: a car's screen cannot fetch the web.
                        .setArtworkUri(track.artwork.url(544).let { ArtworkProvider.uriFor(it) ?: it?.toUri() })
                        .setDurationMs(track.durationMs.takeIf { it > 0 })
                        .setIsPlayable(true)
                        .setIsBrowsable(browsable)
                        .setMediaType(MediaMetadata.MEDIA_TYPE_MUSIC)
                        .setExtras(extras)
                        .build(),
                )
                .build()
        }

        /** The track a media item carries, or one made from its id and names. */
        fun trackOf(item: MediaItem): Track? {
            item.mediaMetadata.extras?.getString(TRACK)?.let { json ->
                runCatching { return CoreJson.decodeFromString(Track.serializer(), json) }
            }
            val id = item.mediaId.takeIf { it.matches(VIDEO_ID) } ?: return null
            val m = item.mediaMetadata
            return Track(id = id, title = m.title?.toString().orEmpty())
        }

        val VIDEO_ID = Regex("^[A-Za-z0-9_-]{11}$")
    }
}

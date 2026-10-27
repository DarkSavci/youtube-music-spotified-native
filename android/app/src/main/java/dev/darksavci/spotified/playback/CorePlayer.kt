package dev.darksavci.spotified.playback

import android.os.Bundle
import android.os.Looper
import androidx.annotation.OptIn
import androidx.core.net.toUri
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
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

    /** Called on the main thread with each projection, and when the engine changes. */
    fun update(next: Projection) {
        projection = next
        invalidateState()
    }

    fun refresh() = invalidateState()

    override fun getState(): State {
        val s = projection.state
        val items = s.queue.items
        val index = s.queue.index.coerceIn(0, (items.size - 1).coerceAtLeast(0))
        val current = items.getOrNull(index)
        val ours = current != null && engine.currentId == current.id
        val playbackState = when {
            items.isEmpty() -> Player.STATE_IDLE
            s.state == PlayState.Loading || s.state == PlayState.Stalled -> Player.STATE_BUFFERING
            ours && engine.isBuffering -> Player.STATE_BUFFERING
            ours -> Player.STATE_READY
            projection.target.playing -> Player.STATE_BUFFERING
            else -> Player.STATE_READY
        }
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
        if (items.isNotEmpty()) {
            builder.setPlaylist(items.mapIndexed { i, t -> itemData(i, t) })
                .setCurrentMediaItemIndex(index)
                .setContentPositionMs {
                    if (engine.currentId == current?.id) engine.positionMs else s.positionMs
                }
                .setIsLoading(playbackState == Player.STATE_BUFFERING)
        }
        return builder.build()
    }

    private fun itemData(index: Int, track: Track): MediaItemData =
        MediaItemData.Builder("$index:${track.id}")
            .setMediaItem(mediaItemOf(track))
            .setDurationUs(if (track.durationMs > 0) track.durationMs * 1000 else C.TIME_UNSET)
            .build()

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        if (playWhenReady != projection.target.playing) playback.toggle()
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
            playback.play(tracks, at, mediaItems.firstOrNull()?.mediaMetadata?.albumTitle?.toString().orEmpty())
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
                        .setArtworkUri(track.artwork.url(544)?.toUri())
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

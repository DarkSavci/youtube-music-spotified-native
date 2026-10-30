package dev.darksavci.spotified.playback

import dev.darksavci.spotified.api.Projection
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch

/*
 * The small rules of who may stop the sound and who may start it again, apart
 * from the system's classes so that they can be tested.
 */

/**
 * This phone gone quiet while its room carries on: what a headset unplugged, a
 * call or a finished sleep timer come to in a room, where a pause would be the
 * whole room's or refused.
 */
enum class Hold {
    /** Another app has the sound for a moment; its going away ends this. */
    ForAMoment,

    /** Stays until the listener asks for the music back. */
    UntilAsked,
}

/** The hold after asking for [asked]: a moment's loss does not soften one the listener has to end. */
fun Hold?.and(asked: Hold): Hold = if (this == Hold.UntilAsked) this else asked

/** The hold once audio focus comes back, which ends only one that came from losing it for a moment. */
fun Hold?.afterFocusBack(): Hold? = if (this == Hold.ForAMoment) null else this

/** A projection as everything but the engine sees it: while held, this phone is paused. */
fun Projection.heldBack(): Projection = copy(target = target.copy(playing = false))

/** Runs work one piece at a time in the order it was posted, off the caller's thread. */
class InOrder(scope: CoroutineScope, private val onError: (Exception) -> Unit = {}) {
    private val work = Channel<suspend () -> Unit>(Channel.UNLIMITED)

    init {
        scope.launch {
            for (piece in work) {
                try {
                    piece()
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    onError(e)
                }
            }
        }
    }

    fun post(piece: suspend () -> Unit) {
        work.trySend(piece)
    }
}

/** What audio focus needs doing, and what is remembered between its changes. */
class FocusBook {
    enum class Step { Nothing, Request, Abandon }

    var held = false
    /** Paused for a call or another app's moment, to carry on after it. */
    var pausedForTransient = false
    private var wanted = false

    /**
     * Whether the music is meant to play has been said again. Play pressed after
     * a pause means the wait for the other app is over. Only the change counts:
     * a projection still saying "playing" as the pause goes out says nothing new.
     */
    fun want(play: Boolean): Step {
        if (play && !wanted) pausedForTransient = false
        wanted = play
        return when {
            play && !held -> Step.Request
            !play && held && !pausedForTransient -> Step.Abandon
            else -> Step.Nothing
        }
    }

    /** Focus came back: whether to carry on. */
    fun regained(): Boolean = pausedForTransient.also { pausedForTransient = false }
}

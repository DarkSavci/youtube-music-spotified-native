package dev.darksavci.spotified.car

import android.content.ComponentName
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.support.v4.media.session.MediaSessionCompat
import androidx.car.app.CarAppService
import androidx.car.app.CarContext
import androidx.car.app.Screen
import androidx.car.app.ScreenManager
import androidx.car.app.Session
import androidx.car.app.media.MediaConstants
import androidx.car.app.media.MediaPlaybackManager
import androidx.car.app.validation.HostValidator
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.lifecycleScope
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import dev.darksavci.spotified.playback.PlaybackService
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import androidx.car.app.R as CarAppR

/**
 * The app's own screens in Android Auto: Home, search, the library and liked
 * songs built from the Car App Library's templates, with Now Playing and the
 * queue on top. Playback stays with [PlaybackService]'s session, which the
 * car is handed here.
 */
class CarService : CarAppService() {
    override fun createHostValidator(): HostValidator =
        if (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0) {
            HostValidator.ALLOW_ALL_HOSTS_VALIDATOR
        } else {
            HostValidator.Builder(this).addAllowedHosts(CarAppR.array.hosts_allowlist_sample).build()
        }

    override fun onCreateSession(): Session = CarSession()
}

class CarSession : Session() {
    private var controller: ListenableFuture<MediaController>? = null
    private val screens get() = carContext.getCarService(ScreenManager::class.java)

    override fun onCreateScreen(intent: Intent): Screen {
        connect()
        lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onDestroy(owner: LifecycleOwner) {
                controller?.let(MediaController::releaseFuture)
            }
        })
        val home = HomeScreen(carContext)
        if (intent.action != MediaConstants.ACTION_SHOW_MEDIA_PLAYBACK) return home
        screens.push(home)
        return NowPlayingScreen(carContext)
    }

    /** The car's own playback card and media buttons go to Now Playing. */
    override fun onNewIntent(intent: Intent) {
        if (intent.action == MediaConstants.ACTION_SHOW_MEDIA_PLAYBACK) showNowPlaying(screens, carContext)
    }

    /**
     * Connecting a controller starts the playback service; its session's
     * token is then handed to the car, which draws Now Playing from it.
     */
    private fun connect() {
        val context = carContext
        controller = MediaController.Builder(context, SessionToken(context, ComponentName(context, PlaybackService::class.java))).buildAsync()
        lifecycleScope.launch {
            val token = PlaybackService.platformToken.filterNotNull().first()
            val manager = context.getCarService(CarContext.MEDIA_PLAYBACK_SERVICE) as MediaPlaybackManager
            manager.registerMediaPlaybackToken(MediaSessionCompat.Token.fromToken(token))
        }
    }
}

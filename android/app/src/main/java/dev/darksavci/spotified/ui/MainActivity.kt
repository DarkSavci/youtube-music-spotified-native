package dev.darksavci.spotified.ui

import android.Manifest
import android.app.SearchManager
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.MediaStore
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.getValue
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.SearchFilter
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.car.AutoSetup
import dev.darksavci.spotified.car.AutoSetupDialog
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.playback.PlaybackService
import dev.darksavci.spotified.ui.theme.SpotifiedTheme
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.mapNotNull
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

class MainActivity : ComponentActivity() {
    private var controller: ListenableFuture<MediaController>? = null
    private var actions: Actions? = null
    private var pending: Intent? = null

    private val askNotifications = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // A screen rebuilt after the process was away must not follow its link again, and play the song again.
        if (savedInstanceState == null) pending = intent
        AutoSetup.front(this, true)
        AutoSetup.watch(this)
        setContent {
            val settings by graph.settings.settings.collectAsStateWithLifecycle(Settings())
            SpotifiedTheme(settings.theme) {
                Root(onReady = { a ->
                    actions = a
                    followPlaylists(a)
                    pending?.let { handle(it) }
                    pending = null
                })
                val asking by AutoSetup.asking.collectAsStateWithLifecycle()
                if (asking) AutoSetupDialog()
            }
        }
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            askNotifications.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    override fun onStart() {
        super.onStart()
        AutoSetup.front(this, true)
        // Connecting a controller starts the playback service, and with it the
        // engine, so a press of play anywhere has something to sound.
        controller = MediaController.Builder(this, SessionToken(this, ComponentName(this, PlaybackService::class.java))).buildAsync()
    }

    override fun onStop() {
        controller?.let(MediaController::releaseFuture)
        controller = null
        AutoSetup.front(this, false)
        super.onStop()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        if (actions != null) handle(intent) else pending = intent
    }

    /** A link, a shared text, a shortcut or the Assistant's "play X". */
    private fun handle(intent: Intent) {
        val a = actions ?: return
        when (intent.action) {
            MediaStore.INTENT_ACTION_MEDIA_PLAY_FROM_SEARCH -> {
                val query = intent.getStringExtra(SearchManager.QUERY).orEmpty()
                lifecycleScope.launch { playSearch(a, query) }
            }
            Intent.ACTION_SEND -> intent.getStringExtra(Intent.EXTRA_TEXT)?.let { open(a, it) }
            Intent.ACTION_VIEW -> intent.data?.let { open(a, it.toString()) }
            ACTION_SIGN_IN -> a.go(Routes.SIGN_IN)
            ACTION_AUTO_SETUP -> AutoSetup.show()
        }
    }

    /** Each playlist page opened becomes the first of the shortcuts on the app's icon. */
    private fun followPlaylists(a: Actions) {
        val entries = a.nav.currentBackStackEntryFlow
            .mapNotNull { entry -> entry.arguments?.getString("id")?.takeIf { entry.destination.route == Routes.PLAYLIST } }
            .distinctUntilChanged()
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) { entries.collect { PlaylistShortcuts.opened(applicationContext, it) } }
        }
    }

    /** A link can open the app, whose session then takes a moment to answer; what it asks for waits that long. */
    private suspend fun sessionReady() {
        withTimeoutOrNull(SESSION_WAIT_MS) { graph.playback.connected.first { it } }
    }

    private fun open(a: Actions, text: String) {
        // What is opened goes on top of everything: a menu or the side menu left up would hide it.
        a.menu = null
        a.drawerOpen = false
        when (val link = Links.read(text)) {
            is Links.Target.Song -> lifecycleScope.launch {
                sessionReady()
                runCatching { graph.api.versions(link.id).firstOrNull { it.id == link.id } }.fold(
                    { track ->
                        // Not every video has a page of versions; the core can still play it by its id.
                        a.play(listOf(track ?: Track(id = link.id)), 0, track?.title.orEmpty())
                        a.nowPlayingOpen = true
                    },
                    { a.say("Couldn’t open that song: ${it.message.orEmpty()}") },
                )
            }
            is Links.Target.Album -> a.go(Routes.album(link.id))
            is Links.Target.Artist -> a.go(Routes.artist(link.id))
            is Links.Target.Playlist -> a.go(Routes.playlist(link.id))
            is Links.Target.Page -> a.go(link.route)
            is Links.Target.Elsewhere -> a.go(Routes.SEARCH + "?q=" + Uri.encode(link.url))
            is Links.Target.Room -> {
                graph.together.openInvite(link.invite)
                a.go(Routes.TOGETHER)
            }
            is Links.Target.Broken -> a.say(link.why)
            null -> when {
                // A link this app has no page for would only search for its address.
                "://" in text -> a.say("This link isn’t a song, album, artist or playlist the app can open.")
                text.isNotBlank() -> a.go(Routes.SEARCH + "?q=" + Uri.encode(text.take(200)))
            }
        }
    }

    private suspend fun playSearch(a: Actions, query: String) {
        if (query.isBlank()) {
            a.playback.resume()
            return
        }
        sessionReady()
        val songs = runCatching {
            graph.api.search(query, SearchFilter.Songs).shelves
                .flatMap { it.items }.filterIsInstance<Item.Song>().map { it.track }
        }.getOrElse {
            a.say("Couldn’t search for \"$query\": ${it.message.orEmpty()}")
            return
        }
        if (songs.isNotEmpty()) a.play(songs.take(25), 0, query) else a.say("Nothing was found for \"$query\".")
    }

    companion object {
        /** Opens sign-in, as a car's "Sign in" asks when something needs an account. */
        const val ACTION_SIGN_IN = "dev.darksavci.spotified.SIGN_IN"

        /** Shows how to make the app appear in Android Auto. */
        const val ACTION_AUTO_SETUP = "dev.darksavci.spotified.AUTO_SETUP"

        private const val SESSION_WAIT_MS = 8000L
    }
}

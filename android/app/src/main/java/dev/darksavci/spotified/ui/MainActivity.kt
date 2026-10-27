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
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.SearchFilter
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.data.Settings
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.playback.PlaybackService
import dev.darksavci.spotified.ui.theme.SpotifiedTheme
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private var controller: ListenableFuture<MediaController>? = null
    private var actions: Actions? = null
    private var pending: Intent? = null

    private val askNotifications = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        pending = intent
        setContent {
            val settings by graph.settings.settings.collectAsStateWithLifecycle(Settings())
            SpotifiedTheme(settings.theme) {
                Root(onReady = { a ->
                    actions = a
                    pending?.let { handle(it) }
                    pending = null
                })
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
        // Connecting a controller starts the playback service, and with it the
        // engine, so a press of play anywhere has something to sound.
        controller = MediaController.Builder(this, SessionToken(this, ComponentName(this, PlaybackService::class.java))).buildAsync()
    }

    override fun onStop() {
        controller?.let(MediaController::releaseFuture)
        controller = null
        super.onStop()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
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
        }
    }

    private fun open(a: Actions, text: String) {
        val link = Links.read(text)
        when (link) {
            is Links.Target.Song -> lifecycleScope.launch {
                val track = runCatching { graph.api.versions(link.id).firstOrNull { it.id == link.id } }.getOrNull()
                a.play(listOf(track ?: Track(id = link.id)), 0, track?.title.orEmpty())
                a.nowPlayingOpen = true
            }
            is Links.Target.Album -> a.go(Routes.album(link.id))
            is Links.Target.Artist -> a.go(Routes.artist(link.id))
            is Links.Target.Playlist -> a.go(Routes.playlist(link.id))
            is Links.Target.Page -> a.go(link.route)
            is Links.Target.Elsewhere -> a.go(Routes.SEARCH + "?q=" + Uri.encode(link.url))
            null -> if (text.isNotBlank()) a.go(Routes.SEARCH + "?q=" + Uri.encode(text.take(200)))
        }
    }

    private suspend fun playSearch(a: Actions, query: String) {
        if (query.isBlank()) {
            a.playback.resume()
            return
        }
        val songs = runCatching {
            graph.api.search(query, SearchFilter.Songs).shelves
                .flatMap { it.items }.filterIsInstance<Item.Song>().map { it.track }
        }.getOrDefault(emptyList())
        if (songs.isNotEmpty()) a.play(songs.take(25), 0, query) else a.say("Nothing was found for \"$query\".")
    }
}

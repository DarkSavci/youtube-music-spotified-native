package dev.darksavci.spotified.spike

import android.os.Bundle
import android.os.SystemClock
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.lifecycleScope
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.util.concurrent.TimeUnit

/**
 * The A0 spike (docs/android.md section 9): the core in-process, yt-dlp
 * under Python with QuickJS, a song through ExoPlayer, and WebView sign-in.
 *
 * Every step is logged under the tag "Spike", so the spike can be driven
 * from adb: `am start -n dev.darksavci.spotified/.spike.MainActivity
 * --es id <videoId> --ez autorun true` resolves and plays without a touch.
 */
class MainActivity : ComponentActivity() {
    private val lines = mutableStateListOf<String>()
    private val address = mutableStateOf<String?>(null)
    private val signingIn = mutableStateOf(false)
    private val videoId = mutableStateOf("kJQP7kiw5Fk")
    // The core refuses requests without its token, which is new at every
    // start, so it is read per request.
    private val http = OkHttpClient.Builder()
        .readTimeout(90, TimeUnit.SECONDS)
        .addInterceptor { chain ->
            val (name, value) = Core.auth()
            chain.proceed(chain.request().newBuilder().header(name, value).build())
        }
        .build()
    private val streams = DefaultHttpDataSource.Factory()
    private lateinit var player: ExoPlayer

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        intent.getStringExtra("id")?.let { videoId.value = it }
        VERBOSE = intent.getBooleanExtra("verbose", false)
        val autorun = intent.getBooleanExtra("autorun", false)

        player = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(streams))
            .build()
        // Tests run muted.
        player.volume = 0f
        player.addListener(object : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) {
                log("player: ${stateName(state)}")
            }

            override fun onPlayerError(error: PlaybackException) {
                log("player error: ${error.errorCodeName}: ${error.message}")
            }
        })

        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(Modifier.fillMaxSize()) {
                    if (signingIn.value) {
                        SignIn(Modifier.fillMaxSize().safeDrawingPadding()) { kept ->
                            signingIn.value = false
                            log("signed in: kept $kept cookies")
                            restartCore()
                        }
                    } else {
                        Screen()
                    }
                }
            }
        }

        lifecycleScope.launch {
            startCore()
            if (autorun) {
                resolve()
                play()
            }
        }
    }

    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        log("new intent: ${intent.extras?.keySet()}")
        val id = intent.getStringExtra("id") ?: return
        videoId.value = id
        if (intent.getBooleanExtra("autorun", false)) {
            lifecycleScope.launch {
                resolve()
                play()
            }
        }
    }

    override fun onDestroy() {
        player.release()
        if (isFinishing) Thread { Core.stop() }.start()
        super.onDestroy()
    }

    @androidx.compose.runtime.Composable
    private fun Screen() {
        Column(Modifier.fillMaxSize().safeDrawingPadding().padding(16.dp)) {
            Text("Core: ${address.value ?: "starting…"}")
            OutlinedTextField(
                value = videoId.value,
                onValueChange = { videoId.value = it.trim() },
                label = { Text("Video id") },
                modifier = Modifier.fillMaxWidth(),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { lifecycleScope.launch { resolve() } }) { Text("Resolve") }
                Button(onClick = { lifecycleScope.launch { play() } }) { Text("Play") }
                Button(onClick = { player.volume = if (player.volume == 0f) 1f else 0f }) {
                    Text("Sound")
                }
                Button(onClick = { signingIn.value = true }) { Text("Sign in") }
            }
            LazyColumn(Modifier.fillMaxSize()) {
                items(lines) { Text(it, style = MaterialTheme.typography.bodySmall) }
            }
        }
    }

    private suspend fun startCore() {
        val started = SystemClock.elapsedRealtime()
        val addr = withContext(Dispatchers.IO) {
            runCatching { Core.start(this@MainActivity) }
        }
        addr.onSuccess {
            address.value = it
            log("core on $it in ${SystemClock.elapsedRealtime() - started} ms")
        }.onFailure { log("core failed: ${it.message}") }
    }

    /** The core writes yt-dlp's cookie file when it starts, so a sign-in restarts it. */
    private fun restartCore() {
        lifecycleScope.launch {
            address.value = null
            withContext(Dispatchers.IO) { Core.stop() }
            startCore()
        }
    }

    private suspend fun resolve() {
        val addr = address.value ?: return log("no core")
        val id = videoId.value
        val started = SystemClock.elapsedRealtime()
        val result = withContext(Dispatchers.IO) {
            runCatching {
                http.newCall(Request.Builder().url("http://$addr/v1/resolve/$id").build())
                    .execute().use { it.code to it.body.string() }
            }
        }
        val ms = SystemClock.elapsedRealtime() - started
        result.onSuccess { (code, body) ->
            if (code != 200) return log("resolve $id: $code $body ($ms ms)")
            val q = JSONObject(body).getJSONObject("quality")
            log("resolve $id: ${q.optString("Label")}, premium ${q.optBoolean("Premium")}, " +
                "${JSONObject(body).optString("mimeType")} ($ms ms)")
        }.onFailure { log("resolve $id failed: ${it.message} ($ms ms)") }
    }

    private suspend fun play() {
        val addr = address.value ?: return log("no core")
        streams.setDefaultRequestProperties(mapOf(Core.auth()))
        player.setMediaItem(MediaItem.fromUri("http://$addr/v1/stream/${videoId.value}"))
        player.prepare()
        player.play()
        val started = SystemClock.elapsedRealtime()
        // Report time to sound and that the position moves.
        while (player.playbackState != Player.STATE_READY && player.playbackState != Player.STATE_IDLE) {
            delay(50)
        }
        log("ready after ${SystemClock.elapsedRealtime() - started} ms")
        repeat(3) {
            delay(2000)
            log("position ${player.currentPosition} ms of ${player.duration} ms, " +
                "playing ${player.isPlaying}")
        }
    }

    private fun log(line: String) {
        Log.i("Spike", line)
        lines.add(0, line)
    }

    private fun stateName(state: Int) = when (state) {
        Player.STATE_IDLE -> "idle"
        Player.STATE_BUFFERING -> "buffering"
        Player.STATE_READY -> "ready"
        Player.STATE_ENDED -> "ended"
        else -> "$state"
    }
}

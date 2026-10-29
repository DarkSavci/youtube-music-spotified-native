package dev.darksavci.spotified

import android.app.Application
import android.content.Context
import dev.darksavci.spotified.api.Api
import dev.darksavci.spotified.api.Session
import dev.darksavci.spotified.core.Core
import dev.darksavci.spotified.data.Crashes
import dev.darksavci.spotified.data.Downloads
import dev.darksavci.spotified.data.ReleasesWorker
import dev.darksavci.spotified.data.Liked
import dev.darksavci.spotified.data.SettingsStore
import dev.darksavci.spotified.data.meteredNetwork
import dev.darksavci.spotified.playback.Playback
import dev.darksavci.spotified.playback.SleepTimer
import dev.darksavci.spotified.together.Together
import dev.darksavci.spotified.widget.PlayerWidget
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import okhttp3.HttpUrl
import okhttp3.OkHttpClient

/**
 * What the app is made of, made once per process: the core, the client that
 * talks to it, the playback session and the settings.
 */
class Graph(context: Context) {
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    val core = Core(context)
    val settings = SettingsStore(context)

    /**
     * Every request to the core carries its token, read per request: it
     * changes when the core restarts. Only to the core's own address and
     * port: another program on the loopback address must never see it.
     */
    val http: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(90, TimeUnit.SECONDS)
        .addInterceptor { chain ->
            val token = core.currentToken()
            val url = chain.request().url
            val request = if (token != null && "${url.host}:${url.port}" == core.currentAddress()) {
                chain.request().newBuilder().header(Core.tokenHeader, token).build()
            } else chain.request()
            chain.proceed(request)
        }
        .also { DebugHooks.install(it, context) }
        .build()

    val origin: suspend () -> HttpUrl = { Api.originOf(core.running().address) }
    val api = Api(http, origin)
    val session = Session(api, Session.streamingClient(http))
    val playback = Playback(scope, session, settings, origin, meteredNetwork(context))
    val liked = Liked(api, scope)
    val sleep = SleepTimer(scope, playback)
    val downloads = Downloads(context, api, scope)
    val together = Together(scope, playback, settings, api, http).also { t ->
        playback.room = { command -> t.state.value.inRoom.also { if (it) t.press(command) } }
    }
}

class App : Application() {
    lateinit var graph: Graph
        private set

    override fun onCreate() {
        super.onCreate()
        Crashes.install(this)
        graph = Graph(this)
        // The core starts at once, beside the first frame, and the session
        // connects as soon as it answers.
        graph.scope.launch {
            runCatching { graph.core.running() }
            graph.playback.start()
            PlayerWidget.follow(this@App, graph.scope)
            ReleasesWorker.schedule(this@App)
            if (graph.downloads.syncedPlaylists.isNotEmpty()) Downloads.schedule(this@App, graph.settings.current().downloadOnWifiOnly)
            graph.liked.refresh()
        }
    }
}

val Context.graph: Graph get() = (applicationContext as App).graph

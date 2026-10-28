package dev.darksavci.spotified

import android.content.Context
import java.io.File
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Protocol
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody

/**
 * What a debug build adds for trying things out on a phone: a song's lyrics
 * answered from files/debug/lyrics/<id>.json when that file is there, so
 * lyrics the sources do not have yet (word timings, say) can be seen
 * playing. Release builds have none of it.
 */
object DebugHooks {
    fun install(builder: OkHttpClient.Builder, context: Context) {
        val dir = File(context.filesDir, "debug/lyrics")
        builder.addInterceptor { chain ->
            val request = chain.request()
            val path = request.url.pathSegments
            val id = path.getOrNull(2)?.takeIf { path.size == 4 && path[0] == "v1" && path[1] == "tracks" && path[3] == "lyrics" }
            val file = id?.let { File(dir, "$it.json") }?.takeIf { it.isFile }
            if (file == null) return@addInterceptor chain.proceed(request)
            Response.Builder().request(request).protocol(Protocol.HTTP_1_1).code(200).message("OK")
                .body(file.readText().toResponseBody("application/json".toMediaType()))
                .build()
        }
    }
}

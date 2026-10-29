package dev.darksavci.spotified

import android.content.Context
import okhttp3.OkHttpClient

/** A release build has no debug hooks; see the debug source set. */
object DebugHooks {
    @Suppress("UNUSED_PARAMETER")
    fun install(builder: OkHttpClient.Builder, context: Context) = Unit
}

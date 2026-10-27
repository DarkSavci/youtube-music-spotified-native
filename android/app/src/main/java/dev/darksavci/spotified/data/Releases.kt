package dev.darksavci.spotified.data

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.core.net.toUri
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.MainActivity
import kotlinx.coroutines.flow.first
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * New releases from the artists in the library, once a day: the newest album
 * or single of each, against what was newest last time. The first look only
 * remembers, so nothing old is announced as new.
 */
class ReleasesWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val graph = applicationContext.graph
        if (!graph.settings.settings.first().newReleaseAlerts) return Result.success()
        graph.core.running()
        val artists = runCatching { graph.api.library().filter { it.kind == LibraryKind.Artist } }.getOrElse { return Result.retry() }
        val file = File(applicationContext.filesDir, "releases.txt")
        val known = if (file.exists()) file.readLines().associate { it.substringBefore('\t') to it.substringAfter('\t') } else emptyMap()
        val first = !file.exists()
        val now = known.toMutableMap()
        for (a in artists.take(60)) {
            val artist = runCatching { graph.api.artist(a.id) }.getOrNull() ?: continue
            val newest = (artist.albums + artist.singles).maxByOrNull { it.year } ?: continue
            if (!first && known[a.id] != null && known[a.id] != newest.id) {
                notify(applicationContext, artist.name, newest.title, newest.id)
            }
            now[a.id] = newest.id
        }
        file.writeText(now.entries.joinToString("") { "${it.key}\t${it.value}\n" })
        return Result.success()
    }

    companion object {
        private const val WORK = "new-releases"
        private const val CHANNEL = "releases"

        fun schedule(context: Context) {
            val work = PeriodicWorkRequestBuilder<ReleasesWorker>(1, TimeUnit.DAYS)
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).setRequiresBatteryNotLow(true).build())
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(WORK, ExistingPeriodicWorkPolicy.KEEP, work)
        }

        private fun notify(context: Context, artist: String, title: String, albumId: String) {
            if (ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED &&
                Build.VERSION.SDK_INT >= 33
            ) return
            val manager = context.getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(NotificationChannel(CHANNEL, "New releases", NotificationManager.IMPORTANCE_DEFAULT))
            val open = PendingIntent.getActivity(
                context, albumId.hashCode(),
                Intent(context, MainActivity::class.java).setAction(Intent.ACTION_VIEW).setData("https://music.youtube.com/browse/$albumId".toUri()),
                PendingIntent.FLAG_IMMUTABLE,
            )
            val n = NotificationCompat.Builder(context, CHANNEL)
                .setSmallIcon(R.drawable.ic_disc_3)
                .setContentTitle("New from $artist")
                .setContentText(title)
                .setContentIntent(open)
                .setAutoCancel(true)
                .build()
            NotificationManagerCompat.from(context).notify(albumId.hashCode(), n)
        }
    }
}

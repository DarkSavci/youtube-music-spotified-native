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
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.ui.MainActivity

/**
 * The notification while songs download. On Android 16 it is a Live Update: a
 * progress bar kept near the top of the shade and in the status bar, with
 * [NotificationCompat.ProgressStyle] doing the drawing. Older phones get the
 * ordinary progress bar. It lasts only while the app's process does: the core
 * doing the downloading lives in that process.
 */
class DownloadNotice(private val context: Context) {
    /** The songs of the run being shown; it grows with what is added and ends when nothing is left to come. */
    private var batch = emptySet<String>()
    private var shown: String? = null

    fun show(tracks: List<Track>, statuses: Map<String, DownloadStatus>, paused: Boolean) {
        val coming = tracks.filter { statuses[it.id].isUnderWay() }
        if (coming.isEmpty()) {
            batch = emptySet()
            if (shown != null) {
                NotificationManagerCompat.from(context).cancel(ID)
                shown = null
            }
            return
        }
        batch = batch + coming.map { it.id }
        val finished = batch.count { statuses[it] == DownloadStatus.Done || statuses[it] == null }
        val percent = (DownloadPlan.overall(batch, statuses) * 100).toInt()
        val current = coming.firstOrNull { statuses[it.id] is DownloadStatus.Active } ?: coming.first()
        val title = if (paused) "Downloads paused" else "Downloading ${finished + 1} of ${batch.size}"
        // Reposted only when it would look different: the system limits how often an app may.
        val look = "$title|$percent|${current.id}"
        if (look == shown) return
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) return
        shown = look
        context.getSystemService(NotificationManager::class.java)
            .createNotificationChannel(NotificationChannel(CHANNEL, "Downloads", NotificationManager.IMPORTANCE_LOW))
        val open = PendingIntent.getActivity(
            context, ID,
            Intent(context, MainActivity::class.java).setAction(Intent.ACTION_VIEW).setData("spotified://downloads".toUri()),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val button = PendingIntent.getBroadcast(
            context, ID,
            Intent(context, Downloads.FromNotice::class.java).setAction(if (paused) ACTION_RESUME else ACTION_PAUSE),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val builder = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_download)
            .setContentTitle(title)
            .setContentText(current.title)
            .setContentIntent(open)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .addAction(
                if (paused) R.drawable.ic_play else R.drawable.ic_pause,
                if (paused) "Resume" else "Pause",
                button,
            )
        if (paused) {
            builder.setOngoing(false).setProgress(100, percent, false)
        } else {
            builder.setOngoing(true)
                .setRequestPromotedOngoing(true)
                .setShortCriticalText("$percent%")
                .setStyle(
                    NotificationCompat.ProgressStyle()
                        .setStyledByProgress(true)
                        .addProgressSegment(NotificationCompat.ProgressStyle.Segment(100))
                        .setProgress(percent),
                )
                // What an older phone draws; Android 16 draws the style instead.
                .setProgress(100, percent, false)
        }
        NotificationManagerCompat.from(context).notify(ID, builder.build())
    }

    private fun DownloadStatus?.isUnderWay() =
        this is DownloadStatus.Active || this is DownloadStatus.Waiting || this is DownloadStatus.Paused

    companion object {
        private const val ID = 4100
        private const val CHANNEL = "downloads"
        const val ACTION_PAUSE = "dev.darksavci.spotified.PAUSE_DOWNLOADS"
        const val ACTION_RESUME = "dev.darksavci.spotified.RESUME_DOWNLOADS"
    }
}

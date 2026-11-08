package dev.darksavci.spotified.data

import android.app.Notification
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat

/**
 * Keeps the app's process going while songs download. The core doing the
 * downloading lives in that process, and with nothing playing and the app in
 * the background, Android would otherwise stop it partway. A data sync service,
 * in the foreground with the download notification itself, for as long as
 * something is coming and the downloads are not paused.
 *
 * Everything here happens on the main thread, where the service's own calls
 * come; [keep] and [release] may be called from any.
 */
class DownloadService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        starting = false
        instance = this
        val shown = notification
        if (shown == null) {
            finish()
            return START_NOT_STICKY
        }
        // Started with startForegroundService, so it goes to the foreground even if no longer wanted, then stops.
        val went = runCatching {
            val type = if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC else 0
            ServiceCompat.startForeground(this, DownloadNotice.ID, shown, type)
        }
        went.onFailure { Logs.w(TAG, "could not go to the foreground", it) }
        if (went.isFailure || !wanted) finish()
        return START_NOT_STICKY
    }

    /** Android 15 limits a data sync service to some hours a day; past them the downloads carry on as before, unguarded. */
    override fun onTimeout(startId: Int, fgsType: Int) {
        Logs.w(TAG, "the system's time for downloads in the background ran out")
        keepNotice = true
        finish()
    }

    override fun onDestroy() {
        if (instance === this) instance = null
        super.onDestroy()
    }

    private fun finish() {
        ServiceCompat.stopForeground(this, if (keepNotice) ServiceCompat.STOP_FOREGROUND_DETACH else ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
        if (instance === this) instance = null
    }

    companion object {
        private const val TAG = "DownloadService"
        private val main = Handler(Looper.getMainLooper())

        private var instance: DownloadService? = null
        private var starting = false
        private var wanted = false
        private var keepNotice = false
        private var notification: Notification? = null

        /** Something is coming: the service runs, showing [shown]. */
        fun keep(context: Context, shown: Notification) {
            val app = context.applicationContext
            main.post {
                notification = shown
                wanted = true
                if (instance != null || starting) return@post
                starting = true
                // Not allowed from the background on Android 12 and later, as after a sync in the night:
                // the downloads then go on as they did before there was a service.
                runCatching { ContextCompat.startForegroundService(app, Intent(app, DownloadService::class.java)) }
                    .onFailure {
                        starting = false
                        Logs.w(TAG, "could not start", it)
                    }
            }
        }

        /** Nothing is coming, or it is paused: the service ends, leaving the notification up when [keepShown]. */
        fun release(keepShown: Boolean) {
            main.post {
                if (!wanted && instance == null) return@post
                wanted = false
                keepNotice = keepShown
                instance?.finish()
            }
        }
    }
}

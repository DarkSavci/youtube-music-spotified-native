package dev.darksavci.spotified.car

import android.annotation.SuppressLint
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.car.app.connection.CarConnection
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.padding
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import dev.darksavci.spotified.R
import dev.darksavci.spotified.ui.MainActivity
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/**
 * Android Auto hides an app not installed from the Play Store until its
 * "Unknown sources" setting is on. A hidden app gets no call from the car, so all
 * it can see is that the phone has connected to one. If it has never been opened
 * in the car by then, the steps are explained once: in the app if it is open,
 * otherwise as a notification that opens them.
 */
object AutoSetup {
    private const val STORE = "auto_setup"
    private const val EXPLAINED = "explained"
    private const val REACHED = "reached"
    private const val CHANNEL = "auto_setup"
    private const val NOTIFICATION = 7301
    private const val ANDROID_AUTO = "com.google.android.projection.gearhead"

    val STEPS = listOf(
        "On this phone, open Settings and search for Android Auto.",
        "Scroll to the bottom and tap Version about ten times, then allow development settings.",
        "Tap the three dots at the top right, then Developer settings.",
        "Turn on Unknown sources.",
        "Connect to the car again. If the app still isn’t there, add it under Customize launcher in Android Auto’s settings.",
    )

    private val _asking = MutableStateFlow(false)

    /** Whether the steps should show in the app now. */
    val asking: StateFlow<Boolean> = _asking

    private var watching = false
    private var inFront = false
    private var onCar = false

    /** The steps are for a phone on a car that has not shown this app, and only ever once. */
    internal fun shouldExplain(onCar: Boolean, explained: Boolean, reached: Boolean) = onCar && !explained && !reached

    /** Watches for a car for as long as the process lasts. Called on the main thread; later calls do nothing. */
    fun watch(context: Context) {
        if (watching) return
        watching = true
        val app = context.applicationContext
        CarConnection(app).type.observeForever { type ->
            onCar = type == CarConnection.CONNECTION_TYPE_PROJECTION
            check(app)
        }
    }

    /** Whether the app's screen is showing, to choose how to say it. Coming to the front also says what could not be said behind it. */
    fun front(context: Context, showing: Boolean) {
        inFront = showing
        if (showing) check(context.applicationContext)
    }

    private fun check(app: Context) {
        val store = app.getSharedPreferences(STORE, Context.MODE_PRIVATE)
        if (shouldExplain(onCar, store.getBoolean(EXPLAINED, false), store.getBoolean(REACHED, false))) {
            if (inFront) _asking.value = true else notifyOf(app)
        }
    }

    /** The car has opened the app, so there is nothing to explain. */
    fun reached(context: Context) {
        context.getSharedPreferences(STORE, Context.MODE_PRIVATE).edit().putBoolean(REACHED, true).apply()
    }

    /** Shows the steps in the app, whether or not they have been shown before, as a Settings row or the notification asks. */
    fun show() {
        _asking.value = true
    }

    /** They have been read. */
    fun done(context: Context) {
        _asking.value = false
        markExplained(context)
    }

    private fun markExplained(context: Context) {
        context.getSharedPreferences(STORE, Context.MODE_PRIVATE).edit().putBoolean(EXPLAINED, true).apply()
    }

    /** Enabled was just asked, which covers the permission. */
    @SuppressLint("MissingPermission")
    private fun notifyOf(context: Context) {
        // Not seen would be not explained: it is said in the app next time instead.
        if (!NotificationManagerCompat.from(context).areNotificationsEnabled()) return
        markExplained(context)
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(CHANNEL, "Android Auto setup", NotificationManager.IMPORTANCE_DEFAULT))
        val open = PendingIntent.getActivity(
            context, NOTIFICATION,
            Intent(context, MainActivity::class.java).setAction(MainActivity.ACTION_AUTO_SETUP).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_music)
            .setContentTitle("Not seeing the app in Android Auto?")
            .setContentText("Android Auto hides apps from outside the Play Store. Tap to see how to show it.")
            .setStyle(NotificationCompat.BigTextStyle().bigText("Android Auto hides apps from outside the Play Store. Tap to see how to show it."))
            .setContentIntent(open)
            .setAutoCancel(true)
            .build()
        NotificationManagerCompat.from(context).notify(NOTIFICATION, notification)
    }

    internal fun androidAuto(context: Context): Intent? = context.packageManager.getLaunchIntentForPackage(ANDROID_AUTO)
}

/** The steps, numbered, with a way to Android Auto where the phone offers one. */
@Composable
fun AutoSetupDialog() {
    val context = LocalContext.current
    val p = LocalPalette.current
    val open = AutoSetup.androidAuto(context)
    AlertDialog(
        onDismissRequest = { AutoSetup.done(context) },
        containerColor = p.overlay,
        titleContentColor = p.text,
        textContentColor = p.secondary,
        title = { Text("Show this app in Android Auto", style = Type.heading) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                Text("Android Auto hides apps from outside the Play Store until you allow them. You only do this once.", style = Type.body)
                AutoSetup.STEPS.forEachIndexed { i, step ->
                    Text("${i + 1}. $step", style = Type.body, modifier = Modifier.padding(top = 8.dp))
                }
            }
        },
        confirmButton = { TextButton({ AutoSetup.done(context) }) { Text("Got it", style = Type.bodyStrong, color = p.accent) } },
        dismissButton = if (open != null) {
            {
                TextButton({
                    AutoSetup.done(context)
                    context.startActivity(open)
                }) { Text("Open Android Auto", style = Type.bodyStrong, color = p.text) }
            }
        } else null,
    )
}

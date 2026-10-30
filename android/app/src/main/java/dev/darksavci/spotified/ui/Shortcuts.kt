package dev.darksavci.spotified.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import dev.darksavci.spotified.R
import dev.darksavci.spotified.data.Logs
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.widget.widgetBitmap

/**
 * The last playlists opened, offered when the app's icon is held down. The
 * system keeps them; each opens through the same link a shared playlist does.
 */
internal object PlaylistShortcuts {
    /** How many are offered, as the plan has it. */
    private const val MOST = 3
    private const val PREFIX = "playlist:"
    private const val ICON_PX = 192

    /** Of the shortcuts [ids], newest first, those to take away when [newest] is put in front. */
    fun overflowing(ids: List<String>, newest: String): List<String> =
        (listOf(newest) + ids.filter { it != newest }).drop(MOST)

    /** Puts the playlist [id] first. Nothing here is worth a message: a shortcut that can't be made is only not offered. */
    suspend fun opened(context: Context, id: String) {
        val name = PREFIX + id
        runCatching {
            val playlist = context.graph.api.playlistPage(id).playlist
            val title = playlist.title.ifEmpty { return }
            val icon = widgetBitmap(context, playlist.artwork, ICON_PX)?.let(IconCompat::createWithBitmap)
                ?: IconCompat.createWithResource(context, R.drawable.shortcut_liked)
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(Actions.playlistLink(id)), context, MainActivity::class.java)
            val shortcut = ShortcutInfoCompat.Builder(context, name)
                .setShortLabel(title.take(SHORT_LABEL))
                .setLongLabel(title)
                .setIcon(icon)
                .setIntent(intent)
                .setRank(0)
                .build()
            val present = ShortcutManagerCompat.getDynamicShortcuts(context).filter { it.id.startsWith(PREFIX) }.sortedBy { it.rank }.map { it.id }
            ShortcutManagerCompat.pushDynamicShortcut(context, shortcut)
            val over = overflowing(present, name)
            if (over.isNotEmpty()) ShortcutManagerCompat.removeDynamicShortcuts(context, over)
        }.onFailure { Logs.w(TAG, "shortcut for a playlist: ${it.message}") }
    }

    private const val SHORT_LABEL = 25
    private const val TAG = "Shortcuts"
}

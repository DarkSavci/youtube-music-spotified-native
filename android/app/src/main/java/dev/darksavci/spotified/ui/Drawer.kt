package dev.darksavci.spotified.ui

import androidx.annotation.DrawableRes
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.darksavci.spotified.R
import dev.darksavci.spotified.api.Account
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

/**
 * The side menu your picture opens on Home, as Spotify's does: who is signed
 * in, then the places that are not tabs.
 */
@Composable
fun SideMenu(actions: Actions) {
    val p = LocalPalette.current
    AnimatedVisibility(actions.drawerOpen, enter = fadeIn(), exit = fadeOut()) {
        Box(
            Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.6f))
                .clickable(remember { MutableInteractionSource() }, null) { actions.drawerOpen = false },
        )
    }
    AnimatedVisibility(actions.drawerOpen, enter = slideInHorizontally { -it }, exit = slideOutHorizontally { -it }) {
        val account by produceState<Account?>(null) { value = runCatching { actions.graph.api.account() }.getOrNull() }
        Column(
            Modifier.fillMaxHeight().fillMaxWidth(0.84f).background(p.panel)
                .clickable(remember { MutableInteractionSource() }, null) {}
                .windowInsetsPadding(WindowInsets.systemBars),
        ) {
            Row(
                Modifier.fillMaxWidth().clickable { go(actions, Routes.SETTINGS) }.padding(horizontal = 16.dp, vertical = 20.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(Modifier.size(52.dp).clip(CircleShape).background(p.accent), contentAlignment = Alignment.Center) {
                    val avatar = account?.avatarUrl.orEmpty()
                    if (avatar.isNotEmpty()) AsyncImage(avatar, null, Modifier.fillMaxSize())
                    else Text(account?.name?.take(1)?.uppercase() ?: "?", style = Type.heading, color = p.onAccent)
                }
                Spacer(Modifier.width(14.dp))
                Column {
                    Text(account?.name?.ifEmpty { null } ?: "Not signed in", style = Type.bodyStrong.copy(fontSize = 18.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text("View settings", style = Type.small, color = p.secondary)
                }
            }
            HorizontalDivider(color = p.outline)
            Spacer(Modifier.height(8.dp))
            Item(R.drawable.ic_plus, "Add account") { go(actions, Routes.SETTINGS) }
            Item(R.drawable.ic_clock, "Recents") { go(actions, Routes.HISTORY) }
            Item(R.drawable.ic_audio_lines, "Your listening") { go(actions, Routes.STATS) }
            Item(R.drawable.ic_gift, "Your year") { go(actions, Routes.RECAP) }
            Item(R.drawable.ic_users, "Listen Together") { go(actions, Routes.TOGETHER) }
            Item(R.drawable.ic_settings, "Settings and privacy") { go(actions, Routes.SETTINGS) }
        }
    }
}

private fun go(actions: Actions, route: String) {
    actions.drawerOpen = false
    actions.go(route)
}

@Composable
private fun Item(@DrawableRes icon: Int, text: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 20.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Glyph(icon, null, size = 24.dp)
        Spacer(Modifier.width(20.dp))
        Text(text, style = Type.body.copy(fontSize = 17.sp))
    }
}

/** Your picture, as the tabs' titles carry it: it opens the side menu. */
@Composable
fun ProfileButton(actions: Actions, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    val account by produceState<Account?>(null) { value = runCatching { actions.graph.api.account() }.getOrNull() }
    Box(
        modifier.size(32.dp).clip(CircleShape).background(p.accent).clickable(onClickLabel = "Your account") { actions.drawerOpen = true },
        contentAlignment = Alignment.Center,
    ) {
        val avatar = account?.avatarUrl.orEmpty()
        if (avatar.isNotEmpty()) AsyncImage(avatar, null, Modifier.fillMaxSize())
        else Text(account?.name?.take(1)?.uppercase() ?: "?", style = Type.label, color = p.onAccent)
    }
}

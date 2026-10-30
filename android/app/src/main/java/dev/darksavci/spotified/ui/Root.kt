package dev.darksavci.spotified.ui

import androidx.core.view.WindowCompat
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.graphics.luminance
import androidx.compose.runtime.SideEffect
import android.app.Activity
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.WindowInsetsSides
import android.content.res.Configuration
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import dev.darksavci.spotified.R
import dev.darksavci.spotified.data.Crashes
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.components.ConfirmDialog
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.components.LocalPlayingId
import dev.darksavci.spotified.ui.components.predictiveBack
import dev.darksavci.spotified.ui.components.predictiveBackLayer
import dev.darksavci.spotified.ui.player.DragSheet
import dev.darksavci.spotified.ui.player.MiniPlayer
import dev.darksavci.spotified.ui.player.NowPlaying
import dev.darksavci.spotified.ui.player.SheetAt
import dev.darksavci.spotified.ui.player.SheetLayer
import dev.darksavci.spotified.ui.player.rememberDragSheet
import dev.darksavci.spotified.ui.screens.AddSongsScreen
import dev.darksavci.spotified.ui.screens.AlbumScreen
import dev.darksavci.spotified.ui.screens.BlockedScreen
import dev.darksavci.spotified.ui.screens.LicencesScreen
import dev.darksavci.spotified.ui.screens.StatDetailScreen
import dev.darksavci.spotified.ui.screens.WhatsNewScreen
import dev.darksavci.spotified.ui.screens.EpisodeProgress
import dev.darksavci.spotified.ui.screens.ArtistScreen
import dev.darksavci.spotified.ui.screens.BrowseScreen
import dev.darksavci.spotified.ui.screens.DownloadsScreen
import dev.darksavci.spotified.ui.screens.EqualizerScreen
import dev.darksavci.spotified.ui.screens.HistoryScreen
import dev.darksavci.spotified.ui.screens.HomeScreen
import dev.darksavci.spotified.ui.screens.ImportScreen
import dev.darksavci.spotified.ui.screens.LibraryScreen
import dev.darksavci.spotified.ui.screens.LikedScreen
import dev.darksavci.spotified.ui.screens.LocalFilesScreen
import dev.darksavci.spotified.ui.screens.PlaylistScreen
import dev.darksavci.spotified.ui.screens.PodcastScreen
import dev.darksavci.spotified.ui.screens.RecapScreen
import dev.darksavci.spotified.ui.screens.SearchScreen
import dev.darksavci.spotified.ui.screens.SettingsScreen
import dev.darksavci.spotified.ui.screens.SignInScreen
import dev.darksavci.spotified.ui.screens.StatsScreen
import dev.darksavci.spotified.ui.screens.TogetherScreen
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.merge

/** The app: the pages, the bottom bar, the mini player and Now Playing over them. */
@Composable
fun Root(onReady: (Actions) -> Unit) {
    val context = LocalContext.current
    val graph = context.graph
    val nav = rememberNavController()
    val actions = remember { Actions(graph, nav, context) }
    val snackbar = remember { SnackbarHostState() }
    val p = LocalPalette.current

    LaunchedEffect(Unit) { onReady(actions) }
    // The system's own light or dark says nothing of the app's theme, or the player's, which is always dark.
    val view = LocalView.current
    val lightBars = p.window.luminance() > 0.5f && !actions.nowPlayingOpen
    SideEffect {
        (context as? Activity)?.window?.let { window ->
            WindowCompat.getInsetsController(window, view).run {
                isAppearanceLightStatusBars = lightBars
                isAppearanceLightNavigationBars = lightBars
            }
        }
    }
    // The newest message replaces one showing, so what was just done is always what is said.
    LaunchedEffect(actions) {
        merge(
            merge(graph.playback.messages, graph.liked.failures).filter { it.isNotBlank() }.map { Notice(it) },
            actions.notices,
        ).collectLatest { notice ->
            val duration = if (notice.action != null) SnackbarDuration.Long else SnackbarDuration.Short
            val result = snackbar.showSnackbar(notice.message, notice.action, duration = duration)
            if (result == SnackbarResult.ActionPerformed) notice.run?.invoke()
        }
    }
    // A like shows at once and is written after: the page of liked songs loads again once that has had time.
    LaunchedEffect(Unit) {
        graph.liked.ids.drop(1).collectLatest {
            delay(1000)
            Changes.say(Change.Liked)
        }
    }
    LaunchedEffect(graph) { EpisodeProgress.follow(context, graph.playback.projection) }
    val playingId = remember(graph) { graph.playback.projection.map { it.state.current?.id }.distinctUntilChanged() }
        .collectAsStateWithLifecycle(null)

    val playerSheet = rememberDragSheet(if (actions.nowPlayingOpen) SheetAt.Open else SheetAt.Closed)
    var miniTop by remember { mutableFloatStateOf(0f) }
    val wide = LocalConfiguration.current.orientation == Configuration.ORIENTATION_LANDSCAPE
    val hasSong by remember(graph) { graph.playback.projection.map { it.state.current != null }.distinctUntilChanged() }
        .collectAsStateWithLifecycle(false)

    CompositionLocalProvider(LocalActions provides actions, LocalPlayingId provides playingId) {
        Box(Modifier.fillMaxSize().background(p.window)) {
            Column(Modifier.fillMaxSize()) {
                // A phone on its side keeps its bars and any cutout at one end of the screen.
                Box(Modifier.weight(1f).windowInsetsPadding(WindowInsets.navigationBars.union(WindowInsets.displayCutout).only(WindowInsetsSides.Horizontal))) {
                    NavHost(
                        nav,
                        startDestination = Routes.HOME,
                        enterTransition = { if (actions.reduceMotion) EnterTransition.None else if (actions.lastMoveWasTab) TAB_IN else PAGE_IN },
                        exitTransition = { if (actions.reduceMotion) ExitTransition.None else if (actions.lastMoveWasTab) TAB_OUT else PAGE_UNDER },
                        popEnterTransition = { if (actions.reduceMotion) EnterTransition.None else if (actions.lastMoveWasTab) TAB_IN else PAGE_BACK },
                        popExitTransition = { if (actions.reduceMotion) ExitTransition.None else if (actions.lastMoveWasTab) TAB_OUT else PAGE_AWAY },
                    ) {
                        composable(Routes.HOME) { HomeScreen() }
                        composable(
                            Routes.SEARCH_ROUTE,
                            arguments = listOf(navArgument("q") { type = NavType.StringType; defaultValue = "" }),
                        ) { SearchScreen(it.arguments?.getString("q").orEmpty()) }
                        composable(Routes.LIBRARY) { LibraryScreen() }
                        composable(Routes.ALBUM) { AlbumScreen(it.arguments?.getString("id").orEmpty()) }
                        composable(Routes.ARTIST) { ArtistScreen(it.arguments?.getString("id").orEmpty()) }
                        composable(Routes.PLAYLIST) { PlaylistScreen(it.arguments?.getString("id").orEmpty()) }
                        composable(
                            Routes.PLAYLIST_ADD,
                            arguments = listOf(navArgument("title") { type = NavType.StringType; defaultValue = "" }),
                        ) { AddSongsScreen(it.arguments?.getString("id").orEmpty(), it.arguments?.getString("title").orEmpty()) }
                        composable(Routes.PODCAST) { PodcastScreen(it.arguments?.getString("id").orEmpty()) }
                        composable(
                            Routes.BROWSE,
                            arguments = listOf(
                                navArgument("params") { type = NavType.StringType; defaultValue = "" },
                                navArgument("title") { type = NavType.StringType; defaultValue = "" },
                            ),
                        ) {
                            BrowseScreen(
                                it.arguments?.getString("id").orEmpty(),
                                it.arguments?.getString("params").orEmpty(),
                                it.arguments?.getString("title").orEmpty(),
                            )
                        }
                        composable(
                            Routes.LIKED_ROUTE,
                            arguments = listOf(navArgument("shuffle") { type = NavType.StringType; defaultValue = "" }),
                        ) { LikedScreen(shuffle = it.arguments?.getString("shuffle") == "1") }
                        composable(Routes.HISTORY) { HistoryScreen() }
                        composable(Routes.STATS) { StatsScreen() }
                        composable(Routes.SETTINGS) { SettingsScreen() }
                        composable(
                            Routes.SIGN_IN_ROUTE,
                            arguments = listOf(navArgument("new") { type = NavType.StringType; defaultValue = "" }),
                        ) { SignInScreen(new = it.arguments?.getString("new") == "1") }
                        composable(Routes.BLOCKED) { BlockedScreen() }
                        composable(Routes.WHATS_NEW) { WhatsNewScreen() }
                        composable(Routes.LICENCES) { LicencesScreen() }
                        composable(
                            Routes.STAT_ROUTE,
                            arguments = listOf(navArgument("id") { type = NavType.StringType; defaultValue = "" }),
                        ) { StatDetailScreen(it.arguments?.getString("kind").orEmpty(), it.arguments?.getString("id").orEmpty()) }
                        composable(Routes.EQUALIZER) { EqualizerScreen() }
                        composable(Routes.DOWNLOADS) { DownloadsScreen() }
                        composable(Routes.LOCAL) { LocalFilesScreen() }
                        composable(Routes.TOGETHER) { TogetherScreen() }
                        composable(Routes.IMPORT) { ImportScreen() }
                        composable(Routes.RECAP) { RecapScreen() }
                    }
                }
                Box(Modifier.background(Brush.verticalGradient(listOf(Color.Transparent, p.window.copy(alpha = 0.96f), p.window)))) {
                    if (wide) {
                        // Little height to spare: the mini player and the tabs share one short line.
                        Row(
                            Modifier.fillMaxWidth().onGloballyPositioned { miniTop = it.positionInRoot().y }
                                .windowInsetsPadding(WindowInsets.navigationBars.union(WindowInsets.displayCutout).only(WindowInsetsSides.Horizontal + WindowInsetsSides.Bottom)),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            if (hasSong) MiniPlayer(playerSheet, onOpen = { actions.nowPlayingOpen = true }, Modifier.weight(1.2f))
                            BottomBar(actions, Modifier.weight(1f), compact = true)
                        }
                    } else {
                        Column(Modifier.onGloballyPositioned { miniTop = it.positionInRoot().y }) {
                            MiniPlayer(playerSheet, onOpen = { actions.nowPlayingOpen = true })
                            BottomBar(actions)
                        }
                    }
                }
            }

            NowPlayingSlot(actions, playerSheet, miniTop)

            CrashPrompt()
            Startup(actions)
            SideMenu(actions)

            // A sheet follows the back gesture by itself: ModalBottomSheet has its own handler.
            Menus()
            actions.confirming?.let { asked ->
                ConfirmDialog(
                    asked.title, asked.text, asked.action,
                    onConfirm = { actions.confirming = null; asked.run() },
                    onDismiss = { actions.confirming = null },
                    danger = asked.danger,
                    dismiss = asked.dismiss,
                )
            }

            SnackbarHost(
                snackbar,
                Modifier.align(Alignment.BottomCenter).windowInsetsPadding(WindowInsets.navigationBars).padding(bottom = if (wide) 64.dp else 140.dp),
            ) { data ->
                Snackbar(data, containerColor = p.overlay, contentColor = p.text, actionColor = p.accent)
            }
        }
    }
}

/**
 * Now Playing's layer, over everything: a sheet that rises out of the mini player
 * from [miniTop], following the finger. The topmost layer answers back first:
 * lyrics, then the player.
 */
@Composable
private fun NowPlayingSlot(actions: Actions, sheet: DragSheet, miniTop: Float) {
    val playerBack = predictiveBack(actions.nowPlayingOpen) { actions.nowPlayingOpen = false }
    val lyricsBack = predictiveBack(actions.nowPlayingOpen && actions.lyricsOpen) { actions.lyricsOpen = false }
    SheetLayer(
        sheet,
        open = actions.nowPlayingOpen,
        setOpen = {
            actions.nowPlayingOpen = it
            if (!it) actions.lyricsOpen = false
        },
        modifier = Modifier.predictiveBackLayer { playerBack() + lyricsBack() },
        closedAt = miniTop,
        fade = true,
    ) {
        NowPlaying(onClose = { actions.nowPlayingOpen = false })
    }
}

private const val PAGE_MS = 220

// Pages under a tab move a little from the side and fade, as going deeper and coming back.
private val PAGE_IN = fadeIn(tween(PAGE_MS)) + slideInHorizontally(tween(PAGE_MS)) { it / 10 }
private val PAGE_UNDER = fadeOut(tween(PAGE_MS / 2)) + slideOutHorizontally(tween(PAGE_MS)) { -it / 20 }
private val PAGE_BACK = fadeIn(tween(PAGE_MS)) + slideInHorizontally(tween(PAGE_MS)) { -it / 20 }
private val PAGE_AWAY = fadeOut(tween(PAGE_MS / 2)) + slideOutHorizontally(tween(PAGE_MS)) { it / 10 }

// Tabs sit side by side, not one above another, so changing tab only crossfades.
private val TAB_IN: EnterTransition = fadeIn(tween(PAGE_MS))
private val TAB_OUT: ExitTransition = fadeOut(tween(PAGE_MS / 2))

@Composable
private fun BottomBar(actions: Actions, modifier: Modifier = Modifier, compact: Boolean = false) {
    val entry by actions.nav.currentBackStackEntryAsState()
    // By the tab that owns the page, so the highlight stays on the pages under it.
    val tab = remember(entry) { actions.currentTab() }
    Row(
        if (compact) modifier else modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.navigationBars).padding(top = 4.dp, bottom = 4.dp),
    ) {
        Tab(R.drawable.ic_house, R.drawable.ic_house_filled, "Home", tab == Routes.HOME, compact) { actions.tabTo(Routes.HOME) }
        Tab(R.drawable.ic_search, R.drawable.ic_search_filled, "Search", tab == Routes.SEARCH, compact) { actions.tabTo(Routes.SEARCH) }
        Tab(R.drawable.ic_library, R.drawable.ic_library_filled, "Your Library", tab == Routes.LIBRARY, compact) { actions.tabTo(Routes.LIBRARY) }
        Tab(R.drawable.ic_plus, R.drawable.ic_plus, "Create", null, compact) { actions.menu = MenuTarget.Create }
    }
}

/** A tab, or with [selected] null, a button in the bar that opens something rather than being somewhere. */
@Composable
private fun RowScope.Tab(icon: Int, filled: Int, label: String, selected: Boolean?, compact: Boolean, onClick: () -> Unit) {
    val p = LocalPalette.current
    val on = selected == true
    val press = if (selected != null) Modifier.selectable(selected, role = Role.Tab, onClick = onClick)
    else Modifier.clickable(role = Role.Button, onClick = onClick)
    Column(
        Modifier.weight(1f).then(press).padding(vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        // Without its word on a short line, the tab still says it to those who cannot see it.
        Glyph(if (on) filled else icon, if (compact) label else null, tint = if (on) p.text else p.secondary, size = 24.dp)
        if (!compact) Text(label, style = Type.tiny, color = if (on) p.text else p.secondary)
    }
}

/** After a crash, the offer to send what happened; nothing is sent unless asked. */
@Composable
private fun CrashPrompt() {
    val context = LocalContext.current
    var report by remember { mutableStateOf(Crashes.pending(context)) }
    val shown = report ?: return
    AlertDialog(
        onDismissRequest = { Crashes.dismiss(context); report = null },
        title = { Text("The app closed unexpectedly") },
        text = { Text("Send a report so it can be fixed? It opens a GitHub issue with the error filled in, for you to read before posting. It has the app version, your phone model and the error, nothing else.") },
        confirmButton = { TextButton({ Crashes.send(context, shown); report = null }) { Text("Send report") } },
        dismissButton = { TextButton({ Crashes.dismiss(context); report = null }) { Text("Not now") } },
    )
}

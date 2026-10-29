package dev.darksavci.spotified.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
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
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
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
import dev.darksavci.spotified.graph
import dev.darksavci.spotified.ui.components.Glyph
import dev.darksavci.spotified.ui.player.MiniPlayer
import dev.darksavci.spotified.ui.player.NowPlaying
import dev.darksavci.spotified.ui.screens.AlbumScreen
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
    LaunchedEffect(Unit) {
        merge(graph.playback.messages, graph.liked.failures).collect { message ->
            if (message.isNotBlank()) snackbar.showSnackbar(message)
        }
    }

    CompositionLocalProvider(LocalActions provides actions) {
        Box(Modifier.fillMaxSize().background(p.window)) {
            Column(Modifier.fillMaxSize()) {
                Box(Modifier.weight(1f)) {
                    NavHost(nav, startDestination = Routes.HOME) {
                        composable(Routes.HOME) { HomeScreen() }
                        composable(
                            Routes.SEARCH_ROUTE,
                            arguments = listOf(navArgument("q") { type = NavType.StringType; defaultValue = "" }),
                        ) { SearchScreen(it.arguments?.getString("q").orEmpty()) }
                        composable(Routes.LIBRARY) { LibraryScreen() }
                        composable(Routes.ALBUM) { AlbumScreen(it.arguments?.getString("id").orEmpty()) }
                        composable(Routes.ARTIST) { ArtistScreen(it.arguments?.getString("id").orEmpty()) }
                        composable(Routes.PLAYLIST) { PlaylistScreen(it.arguments?.getString("id").orEmpty()) }
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
                        composable(Routes.SIGN_IN) { SignInScreen() }
                        composable(Routes.EQUALIZER) { EqualizerScreen() }
                        composable(Routes.DOWNLOADS) { DownloadsScreen() }
                        composable(Routes.LOCAL) { LocalFilesScreen() }
                        composable(Routes.TOGETHER) { TogetherScreen() }
                        composable(Routes.IMPORT) { ImportScreen() }
                        composable(Routes.RECAP) { RecapScreen() }
                    }
                }
                Box(Modifier.background(Brush.verticalGradient(listOf(Color.Transparent, p.window.copy(alpha = 0.96f), p.window)))) {
                    Column {
                        MiniPlayer(onOpen = { actions.nowPlayingOpen = true })
                        BottomBar(actions)
                    }
                }
            }

            AnimatedVisibility(
                visible = actions.nowPlayingOpen,
                enter = slideInVertically { it },
                exit = slideOutVertically { it },
            ) {
                NowPlaying(onClose = { actions.nowPlayingOpen = false })
            }
            // The topmost layer closes first: lyrics, then the player.
            BackHandler(actions.nowPlayingOpen) { actions.nowPlayingOpen = false }
            BackHandler(actions.nowPlayingOpen && actions.lyricsOpen) { actions.lyricsOpen = false }

            SideMenu(actions)
            BackHandler(actions.drawerOpen) { actions.drawerOpen = false }

            Menus()

            SnackbarHost(
                snackbar,
                Modifier.align(Alignment.BottomCenter).windowInsetsPadding(WindowInsets.navigationBars).padding(bottom = 140.dp),
            )
        }
    }
}

@Composable
private fun BottomBar(actions: Actions) {
    val p = LocalPalette.current
    val entry by actions.nav.currentBackStackEntryAsState()
    val route = entry?.destination?.route
    Row(
        Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.navigationBars).padding(top = 4.dp, bottom = 4.dp),
    ) {
        Tab(R.drawable.ic_house, "Home", route == Routes.HOME) { tabTo(actions, Routes.HOME) }
        Tab(R.drawable.ic_search, "Search", route == Routes.SEARCH_ROUTE) { tabTo(actions, Routes.SEARCH) }
        Tab(R.drawable.ic_library, "Your Library", route == Routes.LIBRARY) { tabTo(actions, Routes.LIBRARY) }
        Tab(R.drawable.ic_plus, "Create", false) { actions.menu = MenuTarget.Create }
    }
}

/** A tab returns to its page as it was left, and a second tap goes back to its top. */
private fun tabTo(actions: Actions, route: String) {
    actions.nowPlayingOpen = false
    actions.nav.navigate(route) {
        popUpTo(actions.nav.graph.startDestinationId) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
}

@Composable
private fun RowScope.Tab(icon: Int, label: String, selected: Boolean, onClick: () -> Unit) {
    val p = LocalPalette.current
    Column(
        Modifier.weight(1f).clickable(onClickLabel = label, role = Role.Tab, onClick = onClick).padding(vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Glyph(icon, null, tint = if (selected) p.text else p.secondary, size = 24.dp)
        Text(label, style = Type.tiny, color = if (selected) p.text else p.secondary)
    }
}

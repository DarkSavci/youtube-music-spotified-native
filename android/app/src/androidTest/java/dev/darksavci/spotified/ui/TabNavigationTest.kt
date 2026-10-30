package dev.darksavci.spotified.ui

import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.ViewModelStore
import androidx.navigation.NavHostController
import androidx.navigation.compose.ComposeNavigator
import androidx.navigation.compose.composable
import androidx.navigation.createGraph
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The tabs on the real navigation library, for what they depend on is how it files
 * saved stacks, which a test of the arithmetic alone cannot see.
 */
@RunWith(AndroidJUnit4::class)
class TabNavigationTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private lateinit var nav: NavHostController

    @Before
    fun setUp() = instrumentation.runOnMainSync {
        val owner = object : LifecycleOwner {
            val registry = LifecycleRegistry.createUnsafe(this)
            override val lifecycle: Lifecycle get() = registry
        }
        owner.registry.currentState = Lifecycle.State.RESUMED
        nav = NavHostController(instrumentation.targetContext).apply {
            setLifecycleOwner(owner)
            setViewModelStore(ViewModelStore())
            navigatorProvider.addNavigator(ComposeNavigator())
            graph = createGraph(startDestination = Routes.HOME) {
                composable(Routes.HOME) {}
                composable(Routes.SEARCH_ROUTE) {}
                composable(Routes.LIBRARY) {}
                composable(Routes.ALBUM) {}
                composable(Routes.ARTIST) {}
            }
        }
    }

    /** The tab tap as Actions makes it. */
    private fun tap(tab: String) = instrumentation.runOnMainSync {
        val current = tabOf()
        when (val what = tabTap(tab, current, nav.currentBackStackEntry?.destination?.route)) {
            TabTap.ToTop -> Unit
            TabTap.ToRoot -> nav.popBackStack(Routes.rootOf(tab), inclusive = false)
            is TabTap.Switch -> nav.switchTab(tab, what.keepLeft)
        }
    }

    private fun open(route: String) = instrumentation.runOnMainSync { nav.navigate(route) }

    private fun tabOf(): String = when {
        inStack(Routes.SEARCH_ROUTE) -> Routes.SEARCH
        inStack(Routes.LIBRARY) -> Routes.LIBRARY
        else -> Routes.HOME
    }

    private fun inStack(route: String) = runCatching { nav.getBackStackEntry(route) }.isSuccess

    private fun stack(): String {
        var out = ""
        instrumentation.runOnMainSync {
            out = nav.currentBackStack.value.mapNotNull { it.destination.route?.takeIf { _ -> it.destination.parent != null } }.joinToString(" > ")
        }
        return out
    }

    private val home = "home"
    private val search = "home > search?q={q}"
    private val library = "home > library"

    @Test
    fun homeIsReachedFromEitherTabAtItsRoot() {
        assertEquals(home, stack())
        tap(Routes.SEARCH)
        assertEquals(search, stack())
        tap(Routes.HOME)
        assertEquals(home, stack())
        tap(Routes.LIBRARY)
        assertEquals(library, stack())
        tap(Routes.HOME)
        assertEquals(home, stack())
    }

    @Test
    fun homeIsReachedAgainAndAgain() {
        repeat(3) {
            tap(Routes.SEARCH)
            tap(Routes.HOME)
            assertEquals(home, stack())
            tap(Routes.LIBRARY)
            tap(Routes.HOME)
            assertEquals(home, stack())
        }
    }

    @Test
    fun searchAndLibraryGoBetweenEachOtherAtTheirRoots() {
        tap(Routes.SEARCH)
        tap(Routes.LIBRARY)
        assertEquals(library, stack())
        tap(Routes.SEARCH)
        assertEquals(search, stack())
        tap(Routes.LIBRARY)
        assertEquals(library, stack())
    }

    @Test
    fun homeIsReachedFromASubPageOfEitherTab() {
        tap(Routes.SEARCH)
        open(Routes.album("a"))
        tap(Routes.HOME)
        assertEquals(home, stack())
        tap(Routes.LIBRARY)
        open(Routes.artist("b"))
        tap(Routes.HOME)
        assertEquals(home, stack())
    }

    @Test
    fun aTabComesBackAsItWasLeftThroughHome() {
        tap(Routes.SEARCH)
        open(Routes.album("a"))
        tap(Routes.HOME)
        tap(Routes.SEARCH)
        assertEquals("$search > album/{id}", stack())
        tap(Routes.LIBRARY)
        open(Routes.artist("b"))
        tap(Routes.HOME)
        tap(Routes.LIBRARY)
        assertEquals("$library > artist/{id}", stack())
    }

    @Test
    fun aTabComesBackAsItWasLeftFromTheOtherTab() {
        tap(Routes.SEARCH)
        open(Routes.album("a"))
        tap(Routes.LIBRARY)
        open(Routes.artist("b"))
        tap(Routes.SEARCH)
        assertEquals("$search > album/{id}", stack())
        tap(Routes.LIBRARY)
        assertEquals("$library > artist/{id}", stack())
    }

    @Test
    fun theSamePageComesBackNotACopy() {
        tap(Routes.SEARCH)
        open(Routes.album("a"))
        var before = ""
        instrumentation.runOnMainSync { before = nav.currentBackStackEntry!!.id }
        tap(Routes.HOME)
        tap(Routes.SEARCH)
        var after = ""
        instrumentation.runOnMainSync { after = nav.currentBackStackEntry!!.id }
        assertEquals(before, after)
    }

    @Test
    fun aTabFromItsSubPageGoesToItsRoot() {
        tap(Routes.SEARCH)
        open(Routes.album("a"))
        open(Routes.artist("b"))
        tap(Routes.SEARCH)
        assertEquals(search, stack())
        tap(Routes.LIBRARY)
        open(Routes.album("c"))
        tap(Routes.LIBRARY)
        assertEquals(library, stack())
    }

    @Test
    fun homesOwnPagesEndAtItsRootWhenAskedTwiceOrLeft() {
        open(Routes.album("a"))
        tap(Routes.HOME)
        assertEquals(home, stack())
        open(Routes.album("a"))
        tap(Routes.SEARCH)
        assertEquals(search, stack())
        tap(Routes.HOME)
        assertEquals(home, stack())
    }

    @Test
    fun aSearchWithAQueryReplacesTheSavedOne() {
        tap(Routes.SEARCH)
        open(Routes.album("a"))
        tap(Routes.LIBRARY)
        instrumentation.runOnMainSync {
            nav.clearBackStack(Routes.SEARCH_ROUTE)
            nav.navigate("search?q=queen") { popUpTo(nav.graph.startDestinationId) { saveState = true } }
        }
        assertEquals(search, stack())
        tap(Routes.HOME)
        assertEquals(home, stack())
        tap(Routes.SEARCH)
        assertEquals(search, stack())
    }
}

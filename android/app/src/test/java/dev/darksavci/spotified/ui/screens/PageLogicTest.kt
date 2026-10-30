package dev.darksavci.spotified.ui.screens

import dev.darksavci.spotified.api.ApiError
import dev.darksavci.spotified.api.Artist
import dev.darksavci.spotified.api.ArtistRef
import dev.darksavci.spotified.api.FreshRead
import dev.darksavci.spotified.ui.components.Loader
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.cancel
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.runBlocking
import dev.darksavci.spotified.api.BrowsePage
import dev.darksavci.spotified.api.HomeChip
import dev.darksavci.spotified.api.Item
import dev.darksavci.spotified.api.LibraryItem
import dev.darksavci.spotified.api.LibraryKind
import dev.darksavci.spotified.api.SearchResults
import dev.darksavci.spotified.api.Shelf
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.ui.OpenedPages
import dev.darksavci.spotified.ui.Routes
import dev.darksavci.spotified.ui.TabTap
import dev.darksavci.spotified.ui.tabTap
import dev.darksavci.spotified.ui.components.movedPastSlop
import androidx.compose.ui.geometry.Offset
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PageLogicTest {
    @Test
    fun anEpisodePicksUpWhereItWasLeftUnlessHardlyBegunOrAsGoodAsFinished() {
        val hour = 3_600_000L
        assertEquals(600_000L, episodeResumeAt(600_000, hour))
        assertEquals(0L, episodeResumeAt(20_000, hour))
        assertEquals(0L, episodeResumeAt(0, hour))
        assertEquals(0L, episodeResumeAt(hour - 60_000, hour))
        // Of unknown length, all there is to go on is where it was.
        assertEquals(600_000L, episodeResumeAt(600_000, 0))
    }

    private fun t(id: String, title: String = id, playable: Boolean = true, video: Boolean = false, item: String = "") =
        Track(id = id, title = title, playable = playable, isVideo = video, playlistItemId = item)

    // ---------- opening pages ----------

    @Test
    fun aSecondTapOnALinkDoesNotOpenItsPageAgain() {
        val opened = OpenedPages()
        opened.note("entry1", "album/A")
        assertTrue(opened.isShowing("entry1", "album/A"))
    }

    @Test
    fun anAlbumOpenedFromAnAlbumIsANewPage() {
        val opened = OpenedPages()
        opened.note("entry1", "album/A")
        assertFalse(opened.isShowing("entry1", "album/B"))
    }

    @Test
    fun theSamePageOpenedAgainLaterIsANewEntry() {
        val opened = OpenedPages()
        opened.note("entry1", "album/A")
        assertFalse(opened.isShowing("entry2", "album/A"))
        assertFalse(opened.isShowing(null, "album/A"))
    }

    @Test
    fun theLibraryTabGoesBackFromAnArtistToItsRoot() {
        assertEquals(TabTap.ToRoot, tabTap(Routes.LIBRARY, Routes.LIBRARY, Routes.ARTIST))
    }

    // ---------- refreshing ----------

    @Test
    fun onlyAPullAsksPastTheCoresKeptAnswers() = runBlocking {
        val scope = CoroutineScope(Dispatchers.Unconfined)
        val fresh = mutableListOf<Boolean>()
        val loader = Loader({ fresh += currentCoroutineContext()[FreshRead] != null; "page" }, scope)
        loader.reload()
        loader.refreshBehind()
        loader.reload(quietly = true)
        scope.cancel()
        assertEquals(listOf(false, false, false, true), fresh)
    }

    // ---------- taps ----------

    @Test
    fun movingPastTheSlopInAnyDirectionStopsATap() {
        assertFalse(movedPastSlop(Offset(3f, 4f), 5f))
        assertTrue(movedPastSlop(Offset(6f, 0f), 5f))
        assertTrue(movedPastSlop(Offset(0f, -6f), 5f))
        assertTrue(movedPastSlop(Offset(-4f, 4f), 5f))
    }

    // ---------- recent searches ----------

    @Test
    fun aSearchGoesFirstOnceHoweverItWasWritten() {
        assertEquals(listOf("Duman", "other"), rememberedSearch(listOf("other", "duman"), "  Duman "))
    }

    @Test
    fun recentSearchesAreCappedAndNothingIsNoSearch() {
        val many = (1..RECENT_SEARCHES_KEPT).map { "q$it" }
        assertEquals(RECENT_SEARCHES_KEPT, rememberedSearch(many, "new").size)
        assertEquals("new", rememberedSearch(many, "new").first())
        assertEquals(many, rememberedSearch(many, "   "))
        assertEquals(listOf("b"), forgottenSearch(listOf("A", "b"), "a"))
    }

    // ---------- the library ----------

    private fun lib(id: String, kind: LibraryKind = LibraryKind.Album, folder: String = "", pinned: Boolean = false, added: String? = null) =
        LibraryItem(id = id, kind = kind, title = id, folderId = folder, pinned = pinned, addedAt = added)

    @Test
    fun theTopOfTheLibraryLeavesOutWhatIsInAFolder() {
        val items = listOf(lib("a"), lib("b", folder = "F"))
        assertEquals(listOf("a"), libraryShown(items, null, null, "", LibrarySort.Alphabetical).map { it.id })
        assertEquals(listOf("b"), libraryShown(items, null, "F", "", LibrarySort.Alphabetical).map { it.id })
    }

    @Test
    fun aKindOrASearchLooksThroughFoldersToo() {
        val items = listOf(lib("alpha", folder = "F"), lib("beta"), lib("gamma", LibraryKind.Artist))
        assertEquals(listOf("alpha"), libraryShown(items, null, null, "ALP", LibrarySort.Alphabetical).map { it.id })
        assertEquals(listOf("alpha", "beta"), libraryShown(items, LibraryKind.Album, null, "", LibrarySort.Alphabetical).map { it.id })
    }

    @Test
    fun aSearchInAFolderStaysInIt() {
        val items = listOf(lib("alpha", folder = "F"), lib("alps"))
        assertEquals(listOf("alpha"), libraryShown(items, null, "F", "al", LibrarySort.Alphabetical).map { it.id })
    }

    @Test
    fun pinsComeFirstWhateverTheSort() {
        val items = listOf(lib("a", added = "2"), lib("z", pinned = true, added = "1"))
        assertEquals(listOf("z", "a"), libraryShown(items, null, null, "", LibrarySort.Alphabetical).map { it.id })
        assertEquals(listOf("z", "a"), libraryShown(items, null, null, "", LibrarySort.Added).map { it.id })
    }

    @Test
    fun theLibraryFindsAPlaylistWithOrWithoutVL() {
        val items = listOf(lib("PL1", LibraryKind.Playlist))
        assertEquals("PL1", libraryEntry(items, LibraryKind.Playlist, "VLPL1")?.id)
        assertNull(libraryEntry(items, LibraryKind.Album, "PL1"))
    }

    // ---------- what is playing ----------

    @Test
    fun twoListsWithOneTitleAreToldApart() {
        val ours = listOf(t("a"), t("b"), t("c"))
        val theirs = listOf(t("x"), t("y"), t("z"))
        assertTrue(isQueueOf(ours, 0, "Greatest Hits", ours, "Greatest Hits"))
        assertFalse(isQueueOf(ours, 0, "Greatest Hits", theirs, "Greatest Hits"))
    }

    @Test
    fun aSongTakenOutOfTheQueueLeavesItTheList() {
        val list = listOf(t("a"), t("b"), t("c"))
        assertTrue(isQueueOf(listOf(t("a"), t("c")), 1, "L", list, "L"))
    }

    @Test
    fun theQueuePlayingARadioSongPastTheListIsNotTheList() {
        val list = listOf(t("a"), t("b"))
        assertFalse(isQueueOf(listOf(t("a"), t("b"), t("r")), 2, "L", list, "L"))
    }

    @Test
    fun anotherNameIsAnotherQueue() {
        val list = listOf(t("a"))
        assertFalse(isQueueOf(list, 0, "M", list, "L"))
    }

    // ---------- lists of songs ----------

    @Test
    fun aSongTwiceHasTwoKeys() {
        assertEquals(listOf("a#0", "b#0", "a#1"), rowKeys(listOf(t("a"), t("b"), t("a"))))
        assertEquals(listOf("i1", "i2"), rowKeys(listOf(t("a", item = "i1"), t("a", item = "i2"))))
        // A place's id given twice, as a repeated page could, must not crash the list.
        assertEquals(listOf("i1", "i1#1"), rowKeys(listOf(t("a", item = "i1"), t("a", item = "i1"))))
    }

    @Test
    fun chosenSongsKeepTheListsOrder() {
        val tracks = listOf(t("a"), t("b"), t("c"))
        val keys = rowKeys(tracks)
        assertEquals(listOf("a", "c"), chosenSongs(tracks, keys, setOf("c#0", "a#0")).map { it.id })
    }

    @Test
    fun sortsAreViewsAndCustomIsTheList() {
        val tracks = listOf(t("b", "Zed"), t("a", "Alpha"))
        assertEquals(tracks, sortedSongs(tracks, SongSort.Custom))
        assertEquals(listOf("a", "b"), sortedSongs(tracks, SongSort.Title).map { it.id })
        val byArtist = listOf(t("x").copy(artists = listOf(ArtistRef("", "Zed"))), t("y").copy(artists = listOf(ArtistRef("", "Abe"))))
        assertEquals(listOf("y", "x"), sortedSongs(byArtist, SongSort.Artist).map { it.id })
    }

    // ---------- the liked page ----------

    @Test
    fun anUnlikedSongLeavesAtOnceAndAReLikeBringsItBack() {
        val gone = unlikedSince(emptySet(), setOf("a", "b"), setOf("a"))
        assertEquals(setOf("b"), gone)
        assertEquals(emptySet<String>(), unlikedSince(gone, setOf("a"), setOf("a", "b")))
    }

    @Test
    fun aSongTheLikeListHasNotHeardOfStaysOnThePage() {
        // Liked on another device: on the page, not in the app's set of ids.
        val page = listOf(t("elsewhere"), t("b"))
        assertEquals(listOf("elsewhere"), stillLiked(page, setOf("b")).map { it.id })
        assertEquals(page, stillLiked(page, emptySet()))
    }

    // ---------- music videos ----------

    @Test
    fun videosGoFromShelvesUnlessAskedFor() {
        val shelf = Shelf(items = listOf(Item.Song(t("a")), Item.Song(t("v", video = true))))
        assertEquals(1, shelfShown(shelf, false).items.size)
        assertEquals(2, shelfShown(shelf, true).items.size)
    }

    // ---------- search ----------

    @Test
    fun aTopSongPlaysOnIntoWhatIsBesideItAndBelow() {
        val results = SearchResults(
            topResult = Item.Song(t("top")),
            topResultItems = listOf(Item.Song(t("beside")), Item.Song(t("top"))),
            shelves = listOf(Shelf(items = listOf(Item.Song(t("below")), Item.Song(t("beside"))))),
        )
        assertEquals(listOf("top", "beside", "below"), topResultQueue(results).map { it.id })
    }

    @Test
    fun aSongUnderAnArtistPlaysItselfWithTheArtist() {
        val song = { id: String -> Item.Song(Track(id = id, title = id)) }
        val results = SearchResults(
            topResult = Item.ArtistItem(Artist(id = "queen", name = "Queen")),
            topResultItems = listOf(song("a"), song("b"), song("c")),
        )
        val queue = topResultQueue(results)
        val rows = topResultBeside(results).filterIsInstance<Item.Song>().map { it.track }

        rows.forEachIndexed { i, row ->
            assertEquals(i, placeInQueue(queue, row))
            assertEquals(row, queue[i])
            assertEquals("Queen", queue[i].artistNames)
        }
    }

    @Test
    fun aRowCarryingMoreThanTheQueuesCopyStillFindsItsPlace() {
        val queue = listOf(Track(id = "a"), Track(id = "b"))
        assertEquals(1, placeInQueue(queue, Track(id = "b", title = "with more")))
        assertEquals(0, placeInQueue(queue, Track(id = "gone")))
    }

    // ---------- home ----------

    @Test
    fun aMissingPodcastsPageIsAnAnswerAndAFailureIsNot() {
        assertEquals(true, podcastsAnswer(Result.success(BrowsePage(shelves = listOf(Shelf())))))
        assertEquals(false, podcastsAnswer(Result.success(BrowsePage())))
        assertEquals(false, podcastsAnswer(Result.failure(ApiError.Status(404, ""))))
        assertNull(podcastsAnswer(Result.failure(ApiError.Status(502, ""))))
        assertNull(podcastsAnswer(Result.failure(ApiError.RateLimited)))
    }

    @Test
    fun aMoodsPageDoesNotChangeTheChipRow() {
        val all = listOf(HomeChip("Relax", "r"), HomeChip("Energize", "e"))
        val moodPage = BrowsePage(chips = listOf(HomeChip("Relax", "r", selected = true)))
        assertEquals(all, moodsAfter(all, moodPage, onAll = false))
        assertEquals(moodPage.chips, moodsAfter(emptyList(), moodPage, onAll = false))
        assertEquals(all, moodsAfter(all, BrowsePage(), onAll = true))
    }

    // ---------- browse ----------

    @Test
    fun aShowAllPageSaysItsNameOnce() {
        assertEquals("", shelfHeading(Shelf(title = "Listen again"), "Listen again"))
        assertEquals("Other", shelfHeading(Shelf(title = "Other"), "Listen again"))
    }

    // ---------- words ----------

    @Test
    fun playsAndSongsReadRightInTheSingular() {
        assertEquals("once", times(1))
        assertEquals("twice", times(2))
        assertEquals("7 times", times(7))
        assertEquals("1 song", songs(1))
        assertEquals("2 songs", songs(2))
    }

    // ---------- episodes ----------

    @Test
    fun anEpisodeSaysHowLongHowMuchIsLeftOrThatItIsPlayed() {
        val hour = 60 * 60_000L
        assertEquals("1 hr 0 min", episodeLine(0, hour, "x"))
        assertEquals("30 min left", episodeLine(hour / 2, hour, "x"))
        assertEquals("Played", episodeLine(hour - 60_000, hour, "x"))
        assertEquals("x", episodeLine(0, 0, "x"))
        assertEquals(0.5f, episodeFraction(hour / 2, hour), 0.001f)
        assertEquals(1f, episodeFraction(hour * 2, hour), 0.001f)
    }
}

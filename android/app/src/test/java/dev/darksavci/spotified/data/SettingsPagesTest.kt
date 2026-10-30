package dev.darksavci.spotified.data

import dev.darksavci.spotified.api.RemoteQueue
import dev.darksavci.spotified.api.Track
import dev.darksavci.spotified.api.AlbumRef
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RedactTest {
    @Test
    fun aCookieHeaderLosesEverythingAfterItsName() {
        assertEquals("Cookie: <redacted>", Redact.text("Cookie: SID=abc; HSID=def"))
        assertEquals("""{"authorization": <redacted>""", Redact.text("""{"authorization": "SAPISIDHASH 123_abc"}"""))
    }

    @Test
    fun sessionCookiesAreTakenByNameAndOtherPairsAreLeft() {
        assertEquals("jar: SAPISID=<redacted>; theme=dark; __Secure-3PSID=<redacted>", Redact.text("jar: SAPISID=secret; theme=dark; __Secure-3PSID=also"))
        assertEquals("VALID=yes", Redact.text("VALID=yes"))
    }

    @Test
    fun aLineOfACookieFileKeepsItsNameAndLosesItsValue() {
        assertEquals(
            ".youtube.com\tTRUE\t/\tTRUE\t1790000000\tLOGIN_INFO\t<redacted>",
            Redact.text(".youtube.com\tTRUE\t/\tTRUE\t1790000000\tLOGIN_INFO\tAFmmF2swRQ"),
        )
    }

    @Test
    fun aStreamAddressAndTokensAndEmailsAreTaken() {
        assertEquals("playing https://…googlevideo.com/<redacted> ok", Redact.text("playing https://rr3---sn-abc.googlevideo.com/videoplayback?expire=1&sig=x ok"))
        assertEquals("GET /v1/x?id=abc&pot=<redacted>&n=1", Redact.text("GET /v1/x?id=abc&pot=SECRET&n=1"))
        assertEquals("""{"visitorData":"<redacted>","videoId":"dQw4w9WgXcQ"}""", Redact.text("""{"visitorData":"Cgt4","videoId":"dQw4w9WgXcQ"}"""))
        assertEquals("signed in as <email>", Redact.text("signed in as ada.lovelace+music@example.co.uk"))
    }

    @Test
    fun whatALogNeedsIsLeftAlone() {
        val line = "track dQw4w9WgXcQ \"Never Gonna Give You Up\" failed: HTTP Error 403"
        assertEquals(line, Redact.text(line))
    }
}

class ReportTest {
    @Test
    fun theSettingsSentLeaveOutWhoIsListenedWith() {
        val text = Report.settingsOf(Settings(togetherName = "Ada", togetherRoomName = "Secret room", togetherSelected = "relay-1"))
        assertFalse(text.contains("Ada"))
        assertFalse(text.contains("Secret room"))
        assertFalse(text.contains("relay-1"))
        assertTrue(text.contains("gapless: true"))
    }

    @Test
    fun aLogIsScrubbedLineByLine() {
        assertEquals("a\nCookie: <redacted>\nb", Report.scrubbed("a\nCookie: SID=1\nb"))
    }

    @Test
    fun sizesReadAsAPersonReadsThem() {
        assertEquals("512 B", sizeText(512))
        assertEquals("340 MB", sizeText(340L * 1024 * 1024))
        assertEquals("1.5 GB", sizeText(1536L * 1024 * 1024))
    }

    @Test
    fun aResetKeepsWhatIsTheListenersOwn() {
        val blocked = Blocked().set(BlockKind.Song, "abc", "A song", true)
        val reset = Settings(gapless = false, reduceMotion = true, crossfadeMs = 3000, togetherName = "Ada", blocked = blocked).resetKeepingYours()
        assertTrue(reset.gapless)
        assertFalse(reset.reduceMotion)
        assertEquals(0L, reset.crossfadeMs)
        assertEquals("Ada", reset.togetherName)
        assertEquals(blocked, reset.blocked)
    }
}

class CredentialsTest {
    private val cookie = "SAPISID=a; LOGIN_INFO=b; OTHER=c"

    @Test
    fun theWebViewsCookiesAreKeptOnlyWhenSignedIn() {
        assertNull(Credentials.fromCookies("YSC=a; PREF=b"))
        val json = Credentials.fromCookies("SAPISID=a; LOGIN_INFO=b; NOTKEPT=c")
        assertNotNull(json)
        assertTrue(json!!.contains("SAPISID=a; LOGIN_INFO=b"))
        assertFalse(json.contains("NOTKEPT"))
    }

    @Test
    fun theDesktopsFileIsAccepted() {
        val read = Credentials.fromFile("""{"cookie":"$cookie","onBehalfOfUser":"123","extra":{"X-Goog-AuthUser":"1"}}""")
        val json = (read as Credentials.Read.Good).json
        assertEquals("123", Credentials.activeChannel(json))
        assertTrue(json.contains(""""x-goog-authuser":"1""""))
    }

    @Test
    fun aFlatMapOfHeadersIsAccepted() {
        val read = Credentials.fromFile("""{"Cookie":"$cookie","x-goog-authuser":"2"}""")
        assertTrue((read as Credentials.Read.Good).json.contains(""""x-goog-authuser":"2""""))
    }

    @Test
    fun aBadFileIsRefusedWithoutQuotingIt() {
        val secret = "SID=topsecret"
        for (bad in listOf("not json $secret", """{"cookie":"$secret"}""", """{"other":"$secret"}""", """{"cookie":"SAPISID=a; LOGIN_INFO=b\r\nX: $secret"}""")) {
            val read = Credentials.fromFile(bad)
            assertTrue(bad, read is Credentials.Read.Bad)
            assertFalse((read as Credentials.Read.Bad).message.contains("topsecret"))
        }
    }

    @Test
    fun choosingAChannelKeepsTheRestOfTheSignIn() {
        val base = """{"cookie":"c","extra":{"x-goog-authuser":"0"}}"""
        assertEquals("", Credentials.activeChannel(base))
        val chosen = Credentials.withChannel(base, "123")
        assertEquals("123", Credentials.activeChannel(chosen))
        assertTrue(chosen.contains(""""x-goog-authuser":"0""""))
        assertEquals("", Credentials.activeChannel(Credentials.withChannel(chosen, "")))
        assertFalse(Credentials.withChannel(chosen, "").contains("onBehalfOfUser"))
    }
}

class ChangelogTest {
    @Test
    fun aReleaseHasItsDateAndItsChangesByKind() {
        val releases = Changelog.parse(
            "# Changelog\n\nEvery release.\n\n## 0.2.0 — 2026-10-04\n\n### New\n\n- One\n- Two\n\n### Fixed\n\n- Three\n\n## 0.1.0\n\nBehind-the-scenes improvements only.\n",
        )
        assertEquals(listOf("0.2.0", "0.1.0"), releases.map { it.version })
        assertEquals("2026-10-04", releases[0].date)
        assertEquals(listOf("One", "Two"), releases[0].groups[0].items)
        assertEquals("", releases[1].date)
        assertEquals("Behind-the-scenes improvements only.", releases[1].note)
    }

    @Test
    fun theNotesInTheRepositoryReadAndCarryTheirChanges() {
        val releases = Changelog.parse(File("../../CHANGELOG.md").readText())
        assertTrue(releases.size > 3)
        assertTrue(releases.first().groups.isNotEmpty())
    }

    @Test
    fun whatIsShownAfterAnUpdate() {
        val all = Changelog.parse("## 0.6.0\n### New\n- a\n## 0.5.1\n### New\n- b\n## 0.5.0\n### New\n- c\n")
        assertEquals(emptyList<Changelog.Release>(), Changelog.toShow(all, null, "0.6.0", freshInstall = true))
        assertEquals(listOf("0.6.0"), Changelog.toShow(all, null, "0.6.0", freshInstall = false).map { it.version })
        assertEquals(emptyList<Changelog.Release>(), Changelog.toShow(all, "0.6.0", "0.6.0", freshInstall = false))
        assertEquals(listOf("0.6.0", "0.5.1"), Changelog.toShow(all, "0.5.0", "0.6.0", freshInstall = false).map { it.version })
        assertEquals(listOf("0.5.1"), Changelog.toShow(all, "0.5.0", "0.5.1", freshInstall = false).map { it.version })
    }
}

class UpdaterPureTest {
    @Test
    fun thePhonesOwnApkComesFirstAndTheUniversalOneStandsIn() {
        val names = listOf("spotified-0.6.0-android-universal.apk", "spotified-0.6.0-android-arm64-v8a.apk", "SHA256SUMS.txt")
        assertEquals("spotified-0.6.0-android-arm64-v8a.apk", Updater.pickApk(names, "arm64-v8a"))
        assertEquals("spotified-0.6.0-android-universal.apk", Updater.pickApk(names, "riscv64"))
        assertNull(Updater.pickApk(listOf("SHA256SUMS.txt"), "arm64-v8a"))
    }

    @Test
    fun theChecksumIsFoundByTheFilesName() {
        assertEquals("abc", Updater.expectedSum("def  other.apk\nabc  a.apk\n", "a.apk"))
        assertNull(Updater.expectedSum("def  other.apk\n", "a.apk"))
    }

    @Test
    fun theQuietCheckComesOnceADay() {
        val day = 24 * 60 * 60 * 1000L
        assertTrue(Updater.dueForLook(0, day))
        assertFalse(Updater.dueForLook(day, day + 1000))
        // A clock put back does not stop it for ever.
        assertTrue(Updater.dueForLook(day * 2, day))
    }
}

class ExportFileTest {
    @Test
    fun exportifysCsvIsRead() {
        val (name, songs) = ExportFile.read(
            "My mix.csv",
            "\"Track URI\",\"Track Name\",\"Album Name\",\"Artist Name(s)\"\n\"spotify:track:1\",\"Song, with comma\",\"Album\",\"First;Second\"\n",
        )
        assertEquals("My mix", name)
        assertEquals(listOf(Importer.Wanted("Song, with comma", "First")), songs)
    }

    @Test
    fun appleMusicsTabbedUtf16IsRead() {
        val bytes = byteArrayOf(0xFF.toByte(), 0xFE.toByte()) + "Name\tArtist\tAlbum\nÖtesi\tSıla\tX\n".toByteArray(Charsets.UTF_16LE)
        val (_, songs) = ExportFile.read("list.txt", ExportFile.decode(bytes))
        assertEquals(listOf(Importer.Wanted("Ötesi", "Sıla")), songs)
    }

    @Test
    fun spotifysOwnExportIsRead() {
        val text = """{"playlists":[{"name":"Road","items":[{"track":{"trackName":"A","artistName":"B"}},{"episode":{}}]}]}"""
        val (name, songs) = ExportFile.read("Playlist1.json", text)
        assertEquals("Road", name)
        assertEquals(listOf(Importer.Wanted("A", "B")), songs)
    }

    @Test
    fun anM3uIsRead() {
        val (_, songs) = ExportFile.read("a.m3u", "#EXTM3U\n#EXTINF:123,Artist - Title\nfile.mp3\n#EXTINF:5,Alone\nb.mp3\n")
        assertEquals(listOf(Importer.Wanted("Title", "Artist"), Importer.Wanted("Alone", "")), songs)
    }

    @Test
    fun plainLinesAreReadAsPastedOnes() {
        val (_, songs) = ExportFile.read("songs.txt", "Artist - Title\n")
        assertEquals(listOf(Importer.Wanted("Title", "Artist")), songs)
    }

    @Test
    fun quotedFieldsMayHoldQuotesAndLineBreaks() {
        assertEquals(listOf(listOf("a\"b", "c\nd"), listOf("e", "f")), ExportFile.rows("\"a\"\"b\",\"c\nd\"\ne,f\n"))
    }
}

class DownloadPlanTest {
    private fun look(wanted: List<String>, kept: List<KeptSong>, session: DownloadPlan.Session = DownloadPlan.Session(), now: Long = 1000, paused: Boolean = false) =
        DownloadPlan.look(wanted, kept.associateBy { it.id }, paused, session, now, atOnce = 2)

    @Test
    fun onlyAFewAreAskedForAtOnceAndTheRestWait() {
        val r = look(listOf("a", "b", "c"), emptyList())
        assertEquals(listOf("a", "b"), r.ask)
        assertTrue(r.statuses["a"] is DownloadStatus.Active)
        assertTrue(r.statuses["c"] is DownloadStatus.Waiting)
    }

    @Test
    fun aFinishedSongMakesRoomForTheNext() {
        val first = look(listOf("a", "b", "c"), emptyList())
        val next = look(listOf("a", "b", "c"), listOf(KeptSong("a", 10, 10, true)), first.session, now = 2000)
        assertEquals(DownloadStatus.Done, next.statuses["a"])
        assertEquals(listOf("c"), next.ask)
    }

    @Test
    fun aSongMarkedByAnEarlierRunIsAskedAgain() {
        val r = look(listOf("a"), listOf(KeptSong("a", 5, 10, false)))
        assertEquals(listOf("a"), r.ask)
        assertEquals(DownloadStatus.Active(0.5f), r.statuses["a"])
    }

    @Test
    fun aStuckSongIsAskedOnceMoreThenFails() {
        val s0 = look(listOf("a"), emptyList()).session
        val quiet = look(listOf("a"), emptyList(), s0, now = 1000 + DownloadPlan.STUCK_MS - 1)
        assertTrue(quiet.ask.isEmpty())
        val again = look(listOf("a"), emptyList(), quiet.session, now = 1000 + DownloadPlan.STUCK_MS)
        assertEquals(listOf("a"), again.ask)
        val failed = look(listOf("a"), emptyList(), again.session, now = 1000 + DownloadPlan.STUCK_MS * 2)
        assertTrue(failed.ask.isEmpty())
        assertTrue(failed.statuses["a"] is DownloadStatus.Failed)
        // A failed song takes no room, and stays failed until something changes.
        assertTrue(look(listOf("a"), emptyList(), failed.session, now = 1000 + DownloadPlan.STUCK_MS * 9).statuses["a"] is DownloadStatus.Failed)
    }

    @Test
    fun growingIsNotStuck() {
        val s0 = look(listOf("a"), emptyList()).session
        val grew = look(listOf("a"), listOf(KeptSong("a", 5, 10)), s0, now = 1000 + DownloadPlan.STUCK_MS)
        assertTrue(grew.ask.isEmpty())
        assertTrue(grew.statuses["a"] is DownloadStatus.Active)
    }

    @Test
    fun aRetryPutsAFailedSongBackInLine() {
        val failed = DownloadPlan.Session(failed = mapOf("a" to 0L))
        assertTrue(look(listOf("a"), emptyList(), failed).statuses["a"] is DownloadStatus.Failed)
        assertEquals(listOf("a"), look(listOf("a"), emptyList(), DownloadPlan.retry(failed, "a")).ask)
    }

    @Test
    fun aFailedSongThatGrowsAfterAllIsComing() {
        val failed = DownloadPlan.Session(failed = mapOf("a" to 3L))
        assertTrue(look(listOf("a"), listOf(KeptSong("a", 4, 10)), failed).statuses["a"] is DownloadStatus.Active)
    }

    @Test
    fun aPauseAsksForNothingAndAResumeAsksAgain() {
        val first = look(listOf("a", "b", "c"), emptyList())
        val held = look(listOf("a", "b", "c"), emptyList(), DownloadPlan.forgetAsked(first.session), paused = true)
        assertTrue(held.ask.isEmpty())
        assertTrue(held.statuses.values.all { it is DownloadStatus.Paused })
        // One already coming can still finish while paused.
        assertEquals(DownloadStatus.Done, look(listOf("a"), listOf(KeptSong("a", 1, 1, true)), held.session, paused = true).statuses["a"])
        assertEquals(listOf("a", "b"), look(listOf("a", "b", "c"), emptyList(), held.session).ask)
    }

    @Test
    fun theOverallProgressCountsFinishedSongsWhole() {
        val statuses = mapOf("a" to DownloadStatus.Done, "b" to DownloadStatus.Active(0.5f), "c" to DownloadStatus.Waiting(null))
        assertEquals(0.5f, DownloadPlan.overall(listOf("a", "b", "c"), statuses), 0.001f)
        assertEquals(0f, DownloadPlan.overall(emptyList(), statuses), 0f)
    }

    @Test
    fun songsAreGatheredByAlbumWithTheLooseOnesLast() {
        fun t(id: String, album: String?) = Track(id = id, album = album?.let { AlbumRef(it, it) })
        val groups = groupByAlbum(listOf(t("1", null), t("2", "X"), t("3", "Y"), t("4", "X")))
        assertEquals(listOf("X", "Y", "Other songs"), groups.map { it.title })
        assertEquals(listOf("2", "4"), groups[0].tracks.map { it.id })
    }
}

class RemotePickupTest {
    @Test
    fun itStartsAtTheSongThatDeviceWasOn() {
        val q = RemoteQueue(listOf(Track(id = "a"), Track(id = "b", playable = false), Track(id = "c")), index = 1, title = " ")
        val found = RemotePickup.pick(q)!!
        assertEquals(listOf("a", "c"), found.tracks.map { it.id })
        assertEquals(1, found.index)
        assertEquals("YouTube Music", found.origin)
    }

    @Test
    fun nothingPlayableIsNothing() {
        assertNull(RemotePickup.pick(RemoteQueue()))
        assertNull(RemotePickup.pick(RemoteQueue(listOf(Track(id = "a", playable = false)))))
    }
    @Test
    fun onlyTheSettingOnANetworkThatCostsHoldsDownloads() {
        assertTrue(DownloadPlan.heldForWifi(wifiOnly = true, metered = true))
        assertFalse(DownloadPlan.heldForWifi(wifiOnly = false, metered = true))
        assertFalse(DownloadPlan.heldForWifi(wifiOnly = true, metered = false))
    }

    @Test
    fun heldForWifiNothingIsAskedForAndBackOnWifiItIs() {
        val held = DownloadPlan.look(listOf("a"), emptyMap(), paused = DownloadPlan.heldForWifi(true, true), DownloadPlan.Session(), 0)
        assertTrue(held.ask.isEmpty())
        val back = DownloadPlan.look(listOf("a"), emptyMap(), paused = DownloadPlan.heldForWifi(true, false), held.session, 1)
        assertEquals(listOf("a"), back.ask)
    }

    @Test
    fun aChangelogLineLosesItsMarkupAndKeepsItsWords() {
        assertEquals("Run the gradlew task", Changelog.plainText("Run the `gradlew` task"))
        assertEquals("A bold word and a slanted one", Changelog.plainText("A **bold** word and a *slanted* one"))
        assertEquals("See the notes here", Changelog.plainText("See the [notes](https://example.com/a) here"))
        // Marks that close nothing are part of the words.
        assertEquals("2 * 3 and snake_case_name", Changelog.plainText("2 * 3 and snake_case_name"))
    }
}

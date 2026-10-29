package dev.darksavci.spotified.data

import android.os.Build
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.darksavci.spotified.graph
import java.io.File
import java.security.MessageDigest
import kotlinx.coroutines.runBlocking
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Protocol
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The updater against the project's real, published releases, whose newest
 * carries the desktop's files only: an older phone app is told there is
 * nothing for it, and a newer one that it is up to date. Then against a
 * release with an APK, answered in the test: it is downloaded, checked
 * against the release's checksums, and handed to be installed only when
 * they match.
 */
@RunWith(AndroidJUnit4::class)
class UpdaterTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    private fun said(current: String): List<String> = runBlocking {
        val said = mutableListOf<String>()
        Updater.check(context, context.graph.http, { said += it }, current)
        println("Updater at $current said: $said")
        said
    }

    /** The real list, where the phone's releases are the published android-v ones, or none yet. */
    @Test
    fun theRealReleasesAreReadForThePhonesOwn() {
        val said = said("0.0.1")
        assertEquals(1, said.size)
        assertTrue(
            said[0],
            said[0] == "There are no releases for the phone yet." || said[0].startsWith("Downloading version"),
        )
    }

    private val update = File(context.cacheDir, "updates/update.apk")

    @After
    fun clean() {
        update.delete()
    }

    /** A client that answers as GitHub would for a release 9.9.9 with [apk] and [sums]. */
    private fun github(apk: ByteArray, sums: (String) -> String): OkHttpClient {
        val name = "spotified-9.9.9-${Build.SUPPORTED_ABIS.first()}.apk"
        // As GitHub lists them, newest first: a desktop release, a phone beta, the phone release.
        val phone = """{"tag_name":"android-v9.9.9","draft":false,"prerelease":false,"assets":[
            {"name":"$name","browser_download_url":"https://github.com/r/$name"},
            {"name":"SHA256SUMS.txt","browser_download_url":"https://github.com/r/SHA256SUMS.txt"}]}"""
        val release = """[{"tag_name":"v10.0.0","draft":false,"prerelease":false,"assets":[]},
            {"tag_name":"android-v10.0.0-beta.1","draft":false,"prerelease":true,"assets":[]}, $phone]"""
        return context.graph.http.newBuilder().addInterceptor { chain ->
            val url = chain.request().url.toString()
            val body = when {
                chain.request().url.encodedPath.endsWith("/releases") -> release.toByteArray()
                url.endsWith("/SHA256SUMS.txt") -> sums(name).toByteArray()
                url.endsWith(".apk") -> apk
                else -> error("unexpected $url")
            }
            Response.Builder().request(chain.request()).protocol(Protocol.HTTP_1_1).code(200).message("OK")
                .body(body.toResponseBody("application/octet-stream".toMediaType())).build()
        }.build()
    }

    /** Runs the updater and returns what it said and the file it handed to be installed, which is kept, not installed. */
    private fun install(http: OkHttpClient): Pair<List<String>, ByteArray?> = runBlocking {
        var handed: ByteArray? = null
        val said = mutableListOf<String>()
        Updater.check(context, http, { said += it }, "0.0.1") { handed = it.readBytes() }
        said to handed
    }

    @Test
    fun aMatchingApkIsHandedToTheInstaller() {
        val apk = File(context.applicationInfo.sourceDir).readBytes()
        val sum = MessageDigest.getInstance("SHA-256").digest(apk).joinToString("") { "%02x".format(it) }
        val (said, installer) = install(github(apk) { name -> "0000  other.zip\n$sum  $name\n" })
        assertEquals(listOf("Downloading version 9.9.9…"), said)
        assertNotNull("the installer was asked for", installer)
        assertTrue(installer!!.contentEquals(apk))
    }

    /** The phone's newest release is 9.9.9; the desktop's 10.0.0 and a beta are passed over. */
    @Test
    fun aNewerAppIsUpToDate() = runBlocking {
        val said = mutableListOf<String>()
        Updater.check(context, github(ByteArray(0)) { "" }, { said += it }, "9.9.9") { error("nothing to install") }
        assertEquals(listOf("This is the newest version (9.9.9)."), said)
    }

    @Test
    fun aMismatchedApkIsDeletedAndNotInstalled() {
        val (said, installer) = install(github("not the app".toByteArray()) { name -> "${"0".repeat(64)}  $name\n" })
        assertEquals("The download does not match its checksum; it was not installed.", said.last())
        assertNull("the installer was not asked for", installer)
        assertFalse(update.exists())
    }
}

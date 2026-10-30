package dev.darksavci.spotified.data

/** What the app is made with, as NOTICE.md and the build's own files name it. */
object Licences {
    data class Entry(val name: String, val licence: String, val url: String)

    val ALL = listOf(
        Entry("Youtube Music Spotified", "MIT", "https://github.com/DarkSavci/youtube-music-spotified-native"),
        Entry("AndroidX and Jetpack Compose", "Apache 2.0", "https://developer.android.com/jetpack"),
        Entry("Media3", "Apache 2.0", "https://github.com/androidx/media"),
        Entry("Kotlin and kotlinx", "Apache 2.0", "https://kotlinlang.org"),
        Entry("OkHttp", "Apache 2.0", "https://square.github.io/okhttp/"),
        Entry("Coil", "Apache 2.0", "https://coil-kt.github.io/coil/"),
        Entry("Chaquopy", "MIT", "https://chaquo.com/chaquopy/"),
        Entry("Python", "PSF 2.0", "https://www.python.org"),
        Entry("yt-dlp", "Unlicense", "https://github.com/yt-dlp/yt-dlp"),
        Entry("yt-dlp-ejs", "Unlicense", "https://github.com/yt-dlp/ejs"),
        Entry("meriyah, in yt-dlp-ejs", "ISC", "https://github.com/meriyah/meriyah"),
        Entry("astring, in yt-dlp-ejs", "MIT", "https://github.com/davidbonnet/astring"),
        Entry("QuickJS-NG", "MIT", "https://github.com/quickjs-ng/quickjs"),
        Entry("Go", "BSD 3-Clause", "https://go.dev"),
        Entry("kkdai/youtube", "MIT", "https://github.com/kkdai/youtube"),
        Entry("ncruces/go-sqlite3", "MIT", "https://github.com/ncruces/go-sqlite3"),
        Entry("modernc.org/sqlite", "BSD 3-Clause", "https://gitlab.com/cznic/sqlite"),
        Entry("goja", "MIT", "https://github.com/dop251/goja"),
        Entry("go-yaml", "MIT and Apache 2.0", "https://github.com/go-yaml/yaml"),
        Entry("Spotifast, for the palette and layout measures", "MIT", "https://github.com/crmne/spotifast"),
        Entry("Lucide icons", "ISC", "https://lucide.dev"),
        Entry("Inter", "SIL Open Font License 1.1", "https://rsms.me/inter/"),
    )
}

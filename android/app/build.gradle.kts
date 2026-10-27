plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.chaquopy)
}

android {
    namespace = "dev.darksavci.spotified"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.darksavci.spotified"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.0.1-spike"
        // Phones, and x86_64 for emulators; the core and qjs are built for
        // these two only. -Pabis=arm64-v8a builds the phone's alone, which
        // an x86_64 emulator also runs, through its ARM translation: the
        // core cannot run as x86_64 Android at all (docs/android.md, risks).
        val abis = (findProperty("abis") as String?)?.split(",")
            ?: listOf("arm64-v8a", "x86_64")
        ndk { abiFilters += abis }
    }

    buildFeatures { compose = true }

    packaging {
        // qjs is a program shipped as libqjs.so, and a program can only be
        // run from where Android extracts native libraries; left inside the
        // APK it could not be run at all.
        jniLibs { useLegacyPackaging = true }
    }
}

chaquopy {
    defaultConfig {
        version = "3.14"
        pip {
            // yt-dlp-ejs must be the version this yt-dlp's pyproject.toml
            // pins, or yt-dlp ignores it and cannot solve the challenges.
            install("yt-dlp==2026.10.26")
            install("yt-dlp-ejs==0.8.0")
        }
    }
}

dependencies {
    // The Go core, built by scripts/build-core-android.ps1.
    implementation(files("libs/core.aar"))

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.material3)
    implementation(libs.media3.exoplayer)
    implementation(libs.okhttp)
}

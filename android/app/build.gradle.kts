plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.chaquopy)
}

val RELEASES = "https://api.github.com/repos/DarkSavci/youtube-music-spotified-native/releases/latest"

android {
    namespace = "dev.darksavci.spotified"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.darksavci.spotified"
        minSdk = 26
        targetSdk = 36
        // -PversionCode and -PversionName stamp a release's own version.
        versionCode = (findProperty("versionCode") as String?)?.toInt() ?: 1
        versionName = (findProperty("versionName") as String?) ?: "0.5.0"
        buildConfigField("String", "RELEASES_URL", "\"$RELEASES\"")
        // Phones, and x86_64 for emulators; the core and qjs are built for
        // these two only. -Pabis=arm64-v8a builds the phone's alone.
        val abis = (findProperty("abis") as String?)?.split(",")
            ?: listOf("arm64-v8a", "x86_64")
        ndk { abiFilters += abis }
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    buildTypes {
        debug {
            // -PreleasesUrl points a test build's updater at a release served
            // on this machine; a release build always asks GitHub.
            (findProperty("releasesUrl") as String?)?.let { buildConfigField("String", "RELEASES_URL", "\"$it\"") }
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // Signed with the release key when one is configured
            // (keystore.properties); otherwise the debug key, for a local build.
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    packaging {
        // qjs is a program shipped as libqjs.so, and a program can only be
        // run from where Android extracts native libraries; left inside the
        // APK it could not be run at all.
        jniLibs { useLegacyPackaging = true }
    }

    testOptions { unitTests.isReturnDefaultValues = true }
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
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.compose.icons)
    implementation(libs.navigation.compose)
    implementation(libs.lifecycle.viewmodel.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.lifecycle.process)
    implementation(libs.media3.exoplayer)
    implementation(libs.media3.session)
    implementation(libs.car.app)
    implementation(libs.androidx.media)
    implementation(libs.media3.datasource.okhttp)
    implementation(libs.okhttp)
    implementation(libs.serialization.json)
    implementation(libs.coroutines.android)
    implementation(libs.coroutines.guava)
    implementation(libs.coil.compose)
    implementation(libs.coil.okhttp)
    implementation(libs.datastore.preferences)
    implementation(libs.glance.appwidget)
    implementation(libs.work.runtime)
    implementation(libs.palette)

    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.test.runner)
    androidTestImplementation(libs.coroutines.guava)
}

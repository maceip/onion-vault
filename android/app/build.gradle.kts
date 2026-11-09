// The Gradle build of the OnionVault Android application.
//
// The native part of OnionVault is written in Rust and built with
// `cargo-ndk`; the task below drives it and copies the resulting shared
// objects into the APK.

import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    // The Compose compiler plugin of Kotlin 2.x; it has to use the version
    // of the Kotlin plugin.
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "dev.onionvault"
    compileSdk = 35
    ndkVersion = "27.2.12479018"

    defaultConfig {
        applicationId = "dev.onionvault"
        minSdk = 29
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"

        ndk {
            // OnionVault ships a single ABI: the arm64 one.
            abiFilters += listOf("arm64-v8a")
        }

    }

    // There is no CMake project: the whole native part of OnionVault is
    // written in Rust and built by cargo-ndk.  The CEF C++ wrapper
    // (`libcef_dll_wrapper.a`) is cross-compiled once against the CEF
    // Android distribution, see docs/cef-android.md.

    buildFeatures {
        // The user interface is written with Jetpack Compose.
        compose = true
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
        debug {
            isMinifyEnabled = false
        }
    }

    dependencies {
        // The biometric prompt confirms the use of the master wrapping key.
        implementation("androidx.biometric:biometric:1.1.0")
        implementation("androidx.core:core-ktx:1.13.1")

        // --- the user interface ------------------------------------------------
        // Every version of the Compose libraries comes from the same BOM, so
        // that they cannot drift apart.
        val composeBom = platform("androidx.compose:compose-bom:2024.10.01")
        implementation(composeBom)
        androidTestImplementation(composeBom)

        // `PredictiveBackHandler` lives in the activity library.
        implementation("androidx.activity:activity-compose:1.9.3")
        implementation("androidx.compose.foundation:foundation")
        implementation("androidx.compose.material3:material3")
        implementation("androidx.compose.ui:ui")
        implementation("androidx.compose.ui:ui-graphics")
        implementation("androidx.compose.ui:ui-tooling-preview")
        debugImplementation("androidx.compose.ui:ui-tooling")
        debugImplementation("androidx.compose.ui:ui-test-manifest")

        // --- the screenshot tests ----------------------------------------------
        // The previews are rendered on the JVM, so that the continuous
        // integration can prove that every screen inflates without an
        // emulator and without a device.
        testImplementation("junit:junit:4.13.2")
        testImplementation("org.robolectric:robolectric:4.14.1")
        testImplementation("androidx.compose.ui:ui-test-junit4")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    testOptions {
        unitTests {
            // The screenshot tests need the resources of the application.
            isIncludeAndroidResources = true
            isReturnDefaultValues = true
        }
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
            // The CEF runtime is a shared object with its own resources.
            pickFirsts += "**/libcef.so"
        }
        resources {
            excludes += setOf("META-INF/*.kotlin_module", "kotlin/**", "DebugProbesKt.bin")
        }
    }

    sourceSets {
        getByName("main") {
            java.srcDirs("src/main/kotlin")
        }
        getByName("test") {
            java.srcDirs("src/test/kotlin")
        }
    }

    // The CEF resources (cef.pak, icudtl.dat, locales) are unpacked from the
    // assets at runtime; they must not be compressed, otherwise Chromium
    // cannot memory map them.
    androidResources {
        noCompress += listOf("pak", "dat", "bin")
    }
}

/**
 * The root of the CEF Android distribution.
 *
 * The distribution provides `libcef.so` and the C++ wrapper
 * `libcef_dll_wrapper.a`; both are linked into the native library and
 * `libcef.so` is packaged into the APK.
 */
val cefRoot: String? = System.getenv("CEF_ANDROID_ROOT")
    ?: rootProject.file("cef/android-arm64").takeIf { it.exists() }?.absolutePath

/** Build the Rust part of the application with cargo-ndk. */
val cargoBuild by tasks.registering(Exec::class) {
    group = "build"
    description = "Builds the Rust part of OnionVault with cargo-ndk."

    workingDir = rootProject.file("..")
    commandLine(
        "cargo", "ndk",
        "-t", "arm64-v8a",
        // The API level of the NDK: `libnativewindow`, which carries the
        // `ANativeWindow` of the off-screen rendering, exists from API 24;
        // the application supports API 29 and later.
        "--platform", "29",
        "-o", rootProject.file("app/src/main/jniLibs").absolutePath,
        "build", "--release"
    )

    environment("ANDROID_NDK_HOME", System.getenv("ANDROID_NDK_HOME") ?: "")
    environment("CARGO_TARGET_DIR", rootProject.file("../.cargo-target").absolutePath)

    onlyIf {
        System.getenv("ONIONVAULT_SKIP_CARGO") != "1"
    }
}

/** Copy the prebuilt CEF runtime into the APK. */
val copyCefRuntime by tasks.registering(Copy::class) {
    group = "build"
    description = "Copies libcef.so of the CEF Android distribution into the APK."

    from(cefRoot?.let { file("$it/lib/libcef.so") })
    into(rootProject.file("app/src/main/jniLibs/arm64-v8a"))
    onlyIf { cefRoot != null }
}

tasks.named("preBuild") {
    dependsOn(cargoBuild)
    dependsOn(copyCefRuntime)
}

/**
 * The screenshot tests of the previews.
 *
 * Robolectric downloads the Android image of the tests; the image is cached
 * in the local Maven repository, so that the continuous integration does not
 * fetch it again for every build.
 */
tasks.withType<Test>().configureEach {
    // A local image of Android that was downloaded before avoids the rate
    // limit of the Maven mirror; the continuous integration uses the
    // dependency itself.
    // The image of Android is downloaded from a mirror of Maven Central:
    // the canonical mirror limits the number of downloads per address.
    systemProperty(
        "robolectric.dependency.repo.url",
        "https://maven-central.storage-download.googleapis.com/maven2",
    )
    systemProperty("robolectric.dependency.repo.id", "central")
    val localRobolectric = File(System.getProperty("user.home"), ".m2/robolectric-local")
    if (localRobolectric.listFiles()?.any { it.name.endsWith(".jar") } == true) {
        systemProperty("robolectric.dependency.dir", localRobolectric.absolutePath)
        systemProperty("robolectric.offline", "true")
    }
    testLogging {
        events("passed", "skipped", "failed")
    }
}

/**
 * The native library that carries the CEF runtime.
 *
 * The Rust code is linked against `libcef.so`; the loader has to find the
 * library inside the APK, therefore the shared objects are not compressed
 * (see `packaging.jniLibs.useLegacyPackaging` above).
 */

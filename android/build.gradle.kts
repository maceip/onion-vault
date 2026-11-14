// The root build file of the Trap Android application.
plugins {
    id("com.android.application") version "8.7.2" apply false
    id("org.jetbrains.kotlin.android") version "2.0.21" apply false
    // The Compose compiler plugin of Kotlin 2.x; it uses the version of the
    // Kotlin plugin.
    id("org.jetbrains.kotlin.plugin.compose") version "2.0.21" apply false
}

// The Rust part of the application is built by cargo-ndk; the task lives in
// the `app` module.

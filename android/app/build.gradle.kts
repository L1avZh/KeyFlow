import java.util.Properties
import org.gradle.internal.os.OperatingSystem

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

// ---------------------------------------------------------------------
// Rust FFI bridge (crates/keyflow-mobile): built from source on every
// build rather than committing prebuilt .so files or generated Kotlin
// bindings to git, so CI and every developer always compile the exact
// same Rust source the rest of the PR is being reviewed against.
// Requires: `rustup target add aarch64-linux-android armv7-linux-androideabi
// x86_64-linux-android i686-linux-android`, `cargo install cargo-ndk`, and
// ANDROID_NDK_HOME (or an NDK under $ANDROID_HOME/ndk/<version>) — see
// android/README.md.
// ---------------------------------------------------------------------
val keyflowMobileDir = rootProject.file("../crates/keyflow-mobile")
val uniffiGeneratedDir = layout.buildDirectory.dir("generated/uniffi/java")
val rustJniLibsDir = layout.buildDirectory.dir("rustJniLibs")

val cargoNdkBuild = tasks.register<Exec>("cargoNdkBuild") {
    description = "Cross-compiles crates/keyflow-mobile (the UniFFI bridge into keyflow-core) for all four Android ABIs via cargo-ndk."
    workingDir = keyflowMobileDir
    commandLine(
        "cargo", "ndk",
        "-t", "arm64-v8a", "-t", "armeabi-v7a", "-t", "x86_64", "-t", "x86",
        "-o", rustJniLibsDir.get().asFile.absolutePath,
        "build", "--release",
    )
    inputs.dir(keyflowMobileDir.resolve("src"))
    inputs.file(keyflowMobileDir.resolve("Cargo.toml"))
    inputs.dir(rootProject.file("../crates/keyflow-core/src"))
    inputs.file(rootProject.file("../crates/keyflow-core/Cargo.toml"))
    outputs.dir(rustJniLibsDir)
}

val cargoBuildHostForBindgen = tasks.register<Exec>("cargoBuildHostForBindgen") {
    description = "Builds keyflow-mobile for the host platform only — never shipped — purely so uniffi-bindgen can read its embedded UniFFI metadata."
    workingDir = keyflowMobileDir
    commandLine("cargo", "build")
    inputs.dir(keyflowMobileDir.resolve("src"))
    inputs.file(keyflowMobileDir.resolve("Cargo.toml"))
    outputs.dir(keyflowMobileDir.resolve("target/debug"))
}

val generateUniffiBindings = tasks.register<Exec>("generateUniffiBindings") {
    description = "Generates the Kotlin bindings (uniffi.keyflow_mobile.*) from the compiled host library."
    dependsOn(cargoBuildHostForBindgen)
    workingDir = keyflowMobileDir
    val hostLib = when {
        OperatingSystem.current().isMacOsX -> "target/debug/libkeyflow_mobile.dylib"
        OperatingSystem.current().isWindows -> "target/debug/keyflow_mobile.dll"
        else -> "target/debug/libkeyflow_mobile.so"
    }
    doFirst { mkdir(uniffiGeneratedDir.get()) }
    commandLine(
        "cargo", "run", "--quiet", "--features", "uniffi-bindgen", "--bin", "uniffi-bindgen", "--",
        "generate", "--library", hostLib, "--language", "kotlin", "--out-dir", uniffiGeneratedDir.get().asFile.absolutePath,
    )
    inputs.dir(keyflowMobileDir.resolve("target/debug"))
    outputs.dir(uniffiGeneratedDir)
}

tasks.named("preBuild") {
    dependsOn(cargoNdkBuild, generateUniffiBindings)
}

// Local JVM unit tests (`./gradlew :app:testDebugUnitTest`) run on this
// machine's own JVM, not on a device/emulator — pointing JNA at the
// host-platform build (built by cargoBuildHostForBindgen above, e.g. the
// macOS .dylib on this machine) lets them exercise the *real* Rust vault
// engine directly, not a mock, the same way the Rust side's own
// `cargo test` does. Kotlin tests that need this real bridge live in
// `app/src/test/java/.../data/VaultRepositoryTest.kt`.
tasks.withType<Test>().configureEach {
    dependsOn(cargoBuildHostForBindgen)
    systemProperty("jna.library.path", keyflowMobileDir.resolve("target/debug").absolutePath)
}

// Release signing comes from one of, in order: CI environment variables
// (GitHub Actions secrets, see .github/workflows/android.yml) or a local,
// git-ignored `signing.properties` file for a developer's own release
// testing. Neither is committed. If neither is present, release builds
// fall back to the Android debug keystore so `./gradlew assembleRelease`
// still works out of the box for local development/CI smoke-testing —
// that output is NOT suitable for distribution and R8/minification still
// runs, so it still verifies shrinking doesn't break the app.
val signingProps = Properties().apply {
    val f = rootProject.file("signing.properties")
    if (f.exists()) f.inputStream().use { load(it) }
}

fun signingValue(propKey: String, envKey: String): String? =
    signingProps.getProperty(propKey) ?: System.getenv(envKey)

android {
    namespace = "app.keyflow.mobile"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.keyflow.mobile"
        minSdk = 26 // Required anyway: the Android Autofill Framework (used for KeyFlow's
                    // core autofill feature) was introduced in API 26 — there is no lower
                    // minSdk that could support it, so this isn't a separate tradeoff.
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        vectorDrawables.useSupportLibrary = true

        // Restricts packaging to the 4 ABIs keyflow-mobile is actually
        // cross-compiled for (cargoNdkBuild, above). Without this, JNA's
        // AAR also contributes its own long-obsolete armeabi/mips/mips64
        // native libs, which would be dead weight — this app's own
        // native code was never built for them.
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64", "x86")
        }
    }

    signingConfigs {
        val keystorePath = signingValue("KEYFLOW_RELEASE_KEYSTORE", "KEYFLOW_RELEASE_KEYSTORE")
        if (keystorePath != null && rootProject.file(keystorePath).exists()) {
            create("release") {
                storeFile = rootProject.file(keystorePath)
                storePassword = signingValue("KEYFLOW_RELEASE_STORE_PASSWORD", "KEYFLOW_RELEASE_STORE_PASSWORD")
                keyAlias = signingValue("KEYFLOW_RELEASE_KEY_ALIAS", "KEYFLOW_RELEASE_KEY_ALIAS")
                keyPassword = signingValue("KEYFLOW_RELEASE_KEY_PASSWORD", "KEYFLOW_RELEASE_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".debug"
            isDebuggable = true
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDir(rustJniLibsDir)
            kotlin.srcDir(uniffiGeneratedDir)
        }
    }

    lint {
        lintConfig = file("lint.xml")
        // Release builds already fail on lintVitalRelease (a fatal-only
        // subset); the full `lintDebug`/`lint` task is run explicitly in
        // CI and locally as its own gate rather than as a build side effect.
        abortOnError = true
        checkReleaseBuilds = true
    }

    testOptions {
        unitTests {
            isIncludeAndroidResources = true
            isReturnDefaultValues = true
        }
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.core.splashscreen)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.navigation.compose)

    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.ui)
    implementation(libs.androidx.ui.graphics)
    implementation(libs.androidx.ui.tooling.preview)
    implementation(libs.androidx.material3)
    implementation(libs.androidx.material.icons.extended)
    debugImplementation(libs.androidx.ui.tooling)
    debugImplementation(libs.androidx.ui.test.manifest)

    implementation(libs.androidx.biometric)
    implementation(libs.androidx.security.crypto)
    implementation(libs.androidx.datastore.preferences)
    implementation(libs.androidx.autofill)

    implementation(libs.kotlinx.coroutines.android)

    // Android-specific artifact (`@aar`, not the desktop `.jar`) — bundles
    // only the Android native-mapping glue UniFFI's generated Kotlin
    // bindings need, not JNA's desktop (Windows/macOS/Linux) native
    // binaries, which would otherwise bloat the APK with code that can
    // never run on this platform.
    implementation("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.mockk)
    testImplementation(libs.turbine)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.ext.junit)
    // The desktop JNA artifact (not the Android `@aar` above): local unit
    // tests run on this machine's own JVM, not on a device, so they need
    // JNA's host-native dispatch library to load the real, host-compiled
    // keyflow-mobile library — see the `test` task's `jna.library.path`
    // wiring below, which points at exactly that host build.
    testImplementation("net.java.dev.jna:jna:${libs.versions.jna.get()}")

    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.ui.test.junit4)
    androidTestImplementation(libs.mockk.android)
}

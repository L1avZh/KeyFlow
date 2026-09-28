# KeyFlow for Android

A native Android client that reuses KeyFlow's existing Rust security core
(`crates/keyflow-core`) — the same vault format, Argon2id/AES-256-GCM
crypto, password generator, and phishing-resistant domain-matching engine
the desktop app and browser extension already use — rather than
reimplementing any of it in Kotlin.

## Architecture

```
Kotlin (Jetpack Compose UI, ViewModels, Android Keystore/BiometricPrompt,
        Autofill Framework service)
   │  calls into, via generated Kotlin bindings + JNA
   ▼
crates/keyflow-mobile   — a thin UniFFI bridge crate: records/errors that
                           mirror keyflow-core's types, one opaque
                           MobileVault object. No cryptography, no vault-
                           file parsing, no domain-matching logic of its
                           own (see the crate's own module doc comment).
   │
   ▼
crates/keyflow-core     — unchanged; exactly the same crate the desktop
                           app and browser-extension agent socket use.
```

`keyflow-core` is 100% synchronous (no tokio/async), so every call from
Kotlin is dispatched onto `Dispatchers.IO` in `VaultRepository`, never
the UI thread.

See [`ARCHITECTURE.md`](../ARCHITECTURE.md) at the repo root for the
full-project architecture, and `crates/keyflow-mobile/src/lib.rs`'s doc
comments for exactly which functions/types cross the FFI boundary and why.

## Prerequisites

- **JDK 17–21.** Gradle/AGP in this project cannot parse a JDK version
  string newer than what they shipped with. If `java -version` on your
  machine reports something newer (this happened on the machine this was
  built on, where installing Gradle via Homebrew pulled in a JDK newer
  than this AGP version supports as a *dependency*, shadowing the
  previous default), set `JAVA_HOME` to a JDK 17–21 install explicitly
  before running `./gradlew`.
- **Android SDK** — `compileSdk`/`targetSdk` 35, `build-tools;35.0.0`,
  `platform-tools`. Point `ANDROID_HOME` at it, or create
  `android/local.properties` with `sdk.dir=/path/to/sdk` (gitignored).
- **Android NDK** `27.2.12479018` (or update the version in
  `app/build.gradle.kts`'s `cargoNdkBuild` task and this file together).
- **Rust Android targets**:
  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
  ```
- **cargo-ndk**: `cargo install cargo-ndk`

You do **not** need to install Gradle globally — `./gradlew` downloads
the pinned version (8.10.2) itself on first run.

## Building

All commands run from the `android/` directory.

```bash
./gradlew :app:assembleDebug      # debug APK, unsigned-for-development
./gradlew :app:assembleRelease    # release APK (R8 + resource shrinking)
./gradlew :app:bundleRelease      # release AAB (what Play Store wants)
./gradlew :app:lintDebug          # Android lint
./gradlew :app:testDebugUnitTest  # unit tests (see Testing, below)
```

The Rust cross-compilation (`cargoNdkBuild`) and Kotlin-bindings
generation (`generateUniffiBindings`) run automatically as part of
Gradle's `preBuild` — nothing is checked into git for either (see the
top of `app/build.gradle.kts` for why, and `.gitignore`'s `android/`
section). First build will take longer while Rust compiles; subsequent
builds are incremental.

Without release signing configured (see below), `assembleRelease`/
`bundleRelease` fall back to the Android debug keystore automatically —
R8/shrinking still run for real, so this is a genuine build-correctness
check, just not something to actually publish.

## Testing

`./gradlew :app:testDebugUnitTest` runs entirely on your own machine's
JVM (not a device/emulator) — but it isn't testing mocks. JNA is pointed
(via `jna.library.path`, set in `app/build.gradle.kts`'s `Test` task
configuration) at the **host-platform build** of `keyflow-mobile`, so
`VaultRepositoryTest`/`GeneratorViewModelTest` genuinely create an
AES-256-GCM/Argon2id-encrypted vault file on disk, generate real
passwords, etc., through the same Rust engine the shipped app uses —
just running the macOS/Linux/Windows build of it instead of an Android
`.so`, since a JVM unit test has no Android runtime.

**Not run in this environment, honestly**: Compose UI tests and
instrumented tests (`androidTest/`) need a real device or emulator,
which wasn't available where this was built (see the repo's QA process
notes) — the test infrastructure/dependencies for them
(`androidx.compose.ui:ui-test-junit4`, `androidx.test.espresso`) are
wired into `app/build.gradle.kts` and ready to use, but no such tests
have been written or executed yet. Add them under
`app/src/androidTest/java/...` and run with
`./gradlew :app:connectedDebugAndroidTest` once a device/emulator is
available. Real-device Autofill behavior, biometric prompts, and
process-death/lifecycle edge cases likewise need a device — see
`ROADMAP.md`'s Android section for the honest list of what that leaves
untested.

## Project layout

```
android/
├── app/
│   ├── src/main/java/app/keyflow/mobile/
│   │   ├── data/           VaultRepository, AppPreferences (DataStore)
│   │   ├── biometric/      Keystore-gated quick-unlock (BiometricPrompt)
│   │   ├── autofill/       Android Autofill Framework service
│   │   ├── ui/             Compose screens, navigation, theme
│   │   └── MainActivity.kt, KeyFlowApplication.kt
│   ├── src/test/           JVM unit tests (real Rust engine, see above)
│   ├── src/androidTest/    instrumented tests (infra only — see above)
│   └── build.gradle.kts    also owns the Rust cross-compilation tasks
└── gradle/libs.versions.toml   version catalog
```

## Signing a real release

Never commit a keystore. For local release testing, create
`android/signing.properties` (gitignored):

```properties
KEYFLOW_RELEASE_KEYSTORE=/absolute/or/relative/path/to/your.keystore
KEYFLOW_RELEASE_STORE_PASSWORD=...
KEYFLOW_RELEASE_KEY_ALIAS=...
KEYFLOW_RELEASE_KEY_PASSWORD=...
```

For CI/GitHub Releases, see
[`docs/releases/android.md`](../docs/releases/android.md) for the GitHub
Actions secrets `.github/workflows/android.yml` expects, and for what
Google Play Console setup genuinely requires a human with account access
(this repository cannot and does not fake a Play Store submission).

## Known limitations

See the Android section of [`ROADMAP.md`](../ROADMAP.md).

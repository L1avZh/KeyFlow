# Android release process

This document covers exactly what's automated today, what a GitHub
repository owner needs to configure, and — separately and clearly marked
— what genuinely requires a human with Google Play Console access that
no repository automation can do on their behalf.

## What CI already does

`.github/workflows/android.yml` runs on every push/PR to `main`:
cross-compiles the Rust bridge for all 4 Android ABIs, generates the
Kotlin bindings, runs lint and unit tests, and builds both a release AAB
and APK. Without any secrets configured, that release build is signed
with the Android debug keystore automatically (see
`android/app/build.gradle.kts`) — real, so it proves the release build
genuinely works, but **not suitable to publish**.

## Configuring real release signing (GitHub Actions)

1. Generate a release keystore, if you don't already have one:
   ```bash
   keytool -genkeypair -v -keystore release.keystore -alias keyflow \
     -keyalg RSA -keysize 2048 -validity 10000
   ```
   **Back this up somewhere durable and private.** Google Play App
   Signing (recommended, see below) means you only ever need this key to
   *upload*, not to re-derive your app's signature if lost — but losing
   it before enrolling in Play App Signing means you can never update
   the app again under the same package name.
2. In the GitHub repository's Settings → Secrets and variables → Actions,
   add:
   | Secret | Value |
   |---|---|
   | `ANDROID_RELEASE_KEYSTORE_BASE64` | `base64 -i release.keystore \| pbcopy` (or equivalent) |
   | `ANDROID_RELEASE_STORE_PASSWORD` | the keystore password |
   | `ANDROID_RELEASE_KEY_ALIAS` | the key alias (`keyflow` above) |
   | `ANDROID_RELEASE_KEY_PASSWORD` | the key password |
3. Nothing else changes — the same `android.yml` workflow picks these up
   automatically (see its "Decode release keystore" step) and the
   resulting AAB/APK artifact it uploads will be signed for real.

**Never** commit `release.keystore`, `signing.properties`, or any of the
above values into the repository. `.gitignore` already excludes
`android/signing.properties`, `android/*.keystore`, and `android/*.jks`.

## GitHub Releases

Not yet wired into `android.yml` — the existing `desktop-build` job in
`.github/workflows/ci.yml` is the precedent (triggers on a version tag,
uses `tauri-apps/tauri-action` to attach installers to a draft GitHub
Release). A follow-up could add an equivalent tag-triggered step to
`android.yml` that runs `gh release upload` with the AAB/APK once release
signing (above) is configured — not done here since it's genuinely
optional infrastructure work, not something blocked on account access.

## Play Store: what's ready vs. what needs you

**Ready in this repository:**
- A real, signed-when-configured, R8-shrunk release AAB (`bundleRelease`).
- Store listing copy: [`docs/store-listing/`](../store-listing/).
- A Data Safety section draft: [`docs/store-listing/data-safety.md`](../store-listing/data-safety.md).
- Privacy policy content (adapt/host `PRIVACY.md` at a public URL — Play
  Console requires a live URL, not a repository file).

**Requires you, with Google Play Console access — cannot be automated
from this repository:**
1. Create a Google Play Developer account (one-time $25 fee) if you don't have one.
2. Create the app entry in Play Console; choose the category from
   [`docs/store-listing/metadata.md`](../store-listing/metadata.md).
3. Upload the release AAB (from CI's `keyflow-android-release` artifact,
   or a local `./gradlew :app:bundleRelease`).
4. **Enroll in Play App Signing** when prompted (Google's recommended
   default) — Play re-signs your app for distribution with a key Google
   manages, using your upload key only to verify it's really you.
5. Complete the Data Safety form in Play Console using
   [`docs/store-listing/data-safety.md`](../store-listing/data-safety.md)
   as the source answers — Play's own form is the actual submission,
   this file is prep material, not itself a Play Console API payload.
6. Complete the content rating questionnaire (a password manager with no
   ads, user-generated content, or violence should qualify for the
   lowest tier — verify by actually going through Play's questionnaire).
7. Set countries/pricing (free), and submit for review.

This repository does not claim, and will not claim, that any of steps
1–7 have happened. No Play Console credentials exist in this environment
and none were used.

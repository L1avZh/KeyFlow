# Google Play store listing — draft copy

Copy this into Play Console's store listing form. Nothing here is
submitted automatically; this is prep material (see
[`docs/releases/android.md`](../releases/android.md)).

## App name
KeyFlow

## Short description (max 80 characters)
Local-first password manager. No account, no server, no tracking.

## Full description (max 4000 characters)

KeyFlow is a local-first password manager. Your vault is encrypted with
Argon2id and AES-256-GCM and stored only on your device — there is no
KeyFlow server, no account to create, and nothing about your saved
logins is ever sent anywhere.

Features:
• Save and organize unlimited logins, with search and favorites
• Strong password and passphrase generator with honest entropy estimates
• Security dashboard: flags weak, reused, old, and duplicate passwords
• Fingerprint/face unlock (quick unlock), backed by Android Keystore
• Android Autofill integration — fill saved logins into other apps and
  websites, with phishing-resistant domain matching (the same matching
  engine used across KeyFlow's desktop app and browser extension)
• CSV import and JSON export, entirely on-device
• Dark and light themes

KeyFlow is also available as a desktop app (macOS, Windows, Linux) and a
Chrome/Edge/Chromium browser extension, sharing the same vault format
and security model. See https://github.com/L1avZh/KeyFlow.

There is no password reset: your master password is the only thing that
can decrypt your vault, on any platform, and KeyFlow has no way to
recover it if you forget it. Choose one you'll remember.

## Category recommendation
Tools (Play's category for utility/productivity apps; "Productivity" is
a reasonable alternative — verify current category names in Play
Console, as they've been renamed before).

## Content rating
Expected: PEGI 3 / Everyone — no ads, no user-generated content shared
with other users, no violence. Confirm by completing Play Console's own
questionnaire; do not assume this without doing so.

## Website
https://github.com/L1avZh/KeyFlow

## Support email / contact
See [`SUPPORT.md`](../../SUPPORT.md) at the repository root.

## Privacy policy URL
Play Console requires a **live, publicly reachable URL** — a repository
file path does not qualify. Host [`PRIVACY.md`](../../PRIVACY.md)'s
content somewhere public (e.g. GitHub Pages for this repo, or the
project's own site) before submitting, and put that URL here.

## Screenshots / feature graphic
Not generated as part of this work — they require a running app on a
device/emulator to capture real screens (see `android/README.md`'s
Testing section for why no emulator was available in this environment).
Capture these once you have a device: Play requires at minimum 2 phone
screenshots (Onboarding, Vault list, Security dashboard, and Generator
are good choices) and, for the full listing, a 1024×500 feature graphic
using the KeyFlow brand mark (`apps/desktop/src-tauri/icons/icon-source.png`).

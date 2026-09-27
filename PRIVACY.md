# Privacy

## The short version

KeyFlow is local-first. Your vault lives in a single encrypted file on
your own device. There is no account to create, no server KeyFlow talks
to, and no telemetry collected in this release. Nothing about your
credentials, your usage, or your device is sent anywhere by this
software.

## What KeyFlow stores, and where

- **Your vault**: `~/Library/Application Support/app.keyflow.desktop/vault.keyflow`
  on macOS (the OS-appropriate equivalent on Windows/Linux — Tauri's
  `app_data_dir()` resolver picks the platform-correct location).
  Encrypted at rest — see SECURITY.md.
- **UI preferences** (theme, auto-lock timeout, clipboard-clear timeout):
  stored in the desktop webview's `localStorage`, on-device only.
- **The optional quick-unlock secret**: your master password, stored in
  the OS's own secure storage (Keychain/Credential Manager/Secret
  Service) *only if you explicitly enable it* in Settings. See
  SECURITY.md for the trade-off this represents.

Nothing else is written anywhere by KeyFlow.

## What KeyFlow does not do

- No account creation or sign-in of any kind is required to use any
  feature in this release.
- No analytics, crash reporting, or usage telemetry is wired into this
  codebase. (If that ever changes, it will be opt-in, disclosed here
  first, and will never include credential data, full URLs of your
  saved sites, or anything else identifying what you have stored.)
- No network requests are made by the desktop app in this release, for
  any purpose — not for the security dashboard (all analysis is computed
  from your already-decrypted, already-local vault), not for update
  checks (auto-update is not implemented yet — see ROADMAP.md and
  SECURITY.md), not for anything else.
- Passwords, decrypted vault contents, and master passwords are never
  logged, and never leave process memory except as ciphertext written to
  your own vault file.

## If cloud sync is ever added

It isn't today. If it is in the future, ARCHITECTURE.md documents the
required shape: the server must never receive the master password, the
derived encryption key, or plaintext credentials — only ciphertext it
cannot itself decrypt (a "zero-knowledge" design). Sync will be opt-in;
local-only use will always remain fully supported and will always be the
default.

## Exported data

The Settings → Export feature writes your credentials to a file **in
plain text**, because that's the only interoperable way to hand data to
another tool. KeyFlow warns you about this in the UI before writing the
file. That file is your responsibility once it exists on disk — store it
somewhere secure and delete it when you're done with it. KeyFlow does not
retain a copy of anything you export.

## Imported data

CSV import is parsed entirely on-device. Nothing about the file you
import — its contents, its filename, or the fact that you imported
anything — is sent anywhere.

# Roadmap

Honest status: this is what v0.1 actually shipped, and what's genuinely
still missing, in roughly the order it should get picked up. Items are
here because they were consciously deferred, not because they were
forgotten.

## Done in v0.1

- Rust core: Argon2id + AES-256-GCM vault, credential model, phishing-
  resistant domain matching, password/passphrase generator with entropy
  estimation, CSV import/JSON export, security-dashboard analyses.
  47 unit tests, `cargo clippy` clean.
- Tauri desktop app (built/run on macOS this release): onboarding,
  unlock, vault CRUD, favorites, generator UI, security dashboard with
  an in-app autofill-matching tester, settings (theme, auto-lock,
  clipboard timeout, master password change, optional OS-keychain quick
  unlock, export/import), global command palette (⌘⇧K/Ctrl⇧K).
- Original visual identity (key + flow mark), light/dark theme, base
  accessibility (keyboard nav, focus states, `prefers-reduced-motion`
  respected, no color-only status indicators).

## Not done — highest priority first

1. **Browser extension.** The single biggest gap. Architecture is
   documented (ARCHITECTURE.md §4: content-script form detection →
   background script → native messaging → desktop app running the exact
   same `keyflow_core::domain` matching engine). None of it is built.
   Without this, KeyFlow cannot actually autofill a real web page — the
   in-app "Autofill Tester" exercises the matching engine but is not a
   substitute.
2. **Windows and Linux builds.** The Rust/TypeScript code is
   platform-agnostic and Tauri targets all three OSes, but this session
   only had macOS available to build and run on. Before claiming
   cross-platform support in practice: build on Windows and Linux CI
   runners, smoke-test onboarding → vault → generator → lock/unlock on
   each, and fix whatever platform-specific issues surface (there will
   be some — e.g. `keyring`'s Linux Secret Service backend needs a
   running D-Bus session and may behave differently across desktop
   environments).
3. **Packaging & distribution.** No installers exist yet: no signed/
   notarized macOS `.dmg`, no Windows `.msi`/portable build, no
   AppImage/`.deb`/`.rpm`, no Homebrew formula. `cargo tauri build`
   produces an unsigned local bundle only.
4. **Code signing & notarization.** Required before any of the above are
   distributable without OS warnings. Needs real Apple Developer /
   Windows code-signing credentials this environment doesn't have.
5. **Auto-update.** Not implemented. Tauri has a supported updater
   plugin; wiring it up means standing up a signed-release pipeline
   first (see #4) — an unsigned auto-updater would be worse than none.
6. **`cargo audit` in CI**, and running it at least once against this
   dependency tree before a tagged release (see SECURITY.md — not run in
   this session).
7. **Native biometric-gated unlock** (Touch ID / Windows Hello prompting
   on *every* unlock, not just once to enable a convenience feature).
   Today's "quick unlock" (SECURITY.md) stores the master password in the
   OS keychain, which is a real but different, more limited feature.
   True per-use biometric gating needs platform-native code (macOS
   `LocalAuthentication`/`SecAccessControl`, Windows Hello APIs) beyond
   what the cross-platform `keyring` crate exposes.
8. **System tray / menu-bar presence and launch-at-login.** Not
   implemented. Needs `tauri-plugin-autostart` (or equivalent) plus a
   tray icon menu; deferred to keep this release's scope honest rather
   than half-wire it.
9. **Lock-on-system-sleep.** Today's locking is inactivity-timeout,
   manual, and (inherently, since the key lives in process memory)
   on-quit. Detecting OS sleep/resume needs platform-specific hooks
   (macOS `NSWorkspace` notifications, Windows `WM_POWERBROADCAST`,
   Linux `systemd-logind` D-Bus signals) not yet wired up.
10. **Full Public Suffix List integration.** `domain.rs` ships a small,
    hand-picked list of common multi-label suffixes (`co.uk`, `com.au`,
    `github.io`, …) rather than the authoritative, regularly-updated
    Mozilla PSL (thousands of entries). Correct for the large majority of
    real domains; documented as a known simplification in
    THREAT_MODEL.md and `domain.rs`'s own doc comments.
11. **Homograph / confusable-script phishing detection.** IDNA/punycode
    normalization is implemented (so inconsistent representations of the
    *same* domain can't bypass matching), but KeyFlow does not flag a
    visually-confusable different domain (e.g. Cyrillic look-alikes) the
    way a browser address bar might. Documented, not silently assumed
    safe.
12. **Fuzzing.** THREAT_MODEL.md names the domain parser, CSV importer,
    and vault-file deserialization as the highest-value fuzz targets.
    Only hand-written adversarial unit tests exist today, not
    property-based fuzzing (`cargo-fuzz` or `proptest` beyond the one
    dev-dependency already in `Cargo.toml`, which isn't yet used for
    fuzzing).
13. **Multiple vault "profiles"** and full work/personal identity
    separation beyond free-form tags. Today there's exactly one vault
    file per install.
14. **Larger, vetted diceware wordlist.** The built-in passphrase
    wordlist (`wordlist.rs`) is a KeyFlow-curated ~1,977-word list, not a
    dedicated audited diceware list — entropy accounting is computed from
    its real length so the strength meter stays honest either way, but a
    larger/standard list would be a straightforward improvement.
15. **Structured logging with automatic secret redaction**, plus the
    automated tests the project brief calls for verifying secrets can't
    enter logs. There is currently no logging framework wired in at
    all — see SECURITY.md.
16. **CI workflows for Windows/Linux builds and tests** (the checked-in
    `.github/workflows/ci.yml` currently only runs on macOS/ubuntu
    runners for lint+test; full cross-platform *build* verification and
    release artifact generation is future work tied to items #2–#4).

## Explicitly not planned

Per the project brief's own "do not overengineer" guidance: no
cryptocurrency, no social features, no invasive analytics, no AI bolted
on for marketing's sake. Every feature above earns its place by solving
an actual security, reliability, or UX problem this product has.

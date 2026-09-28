# Roadmap

Honest status: this is what v0.1 actually shipped, and what's genuinely
still missing, in roughly the order it should get picked up. Items are
here because they were consciously deferred, not because they were
forgotten.

## Done in v0.1

- Rust core: Argon2id + AES-256-GCM vault, credential model, phishing-
  resistant domain matching, password/passphrase generator with entropy
  estimation, CSV import/JSON export, security-dashboard analyses.
  47 unit tests, `cargo clippy` clean. `cargo audit` runs in CI
  (`.github/workflows/ci.yml`) and has been run against the real
  dependency tree: zero known vulnerabilities (see SECURITY.md).
- Tauri desktop app (built/run on macOS this release): onboarding,
  unlock, vault CRUD, favorites, generator UI, security dashboard with
  an in-app autofill-matching tester, settings (theme, auto-lock,
  clipboard timeout, master password change, optional OS-keychain quick
  unlock, export/import), global command palette (⌘⇧K/Ctrl⇧K).
- Original visual identity (key + flow mark), light/dark theme, base
  accessibility (keyboard nav, focus states, `prefers-reduced-motion`
  respected, no color-only status indicators).
- **(Added 2026-09-28) Browser extension for Chrome/Edge/Chromium (MV3)**
  — `browser-extension/chrome`, plus two new crates: `keyflow-agent`
  (shared protocol/path resolution/native-messaging framing, 4 unit
  tests) and `keyflow-native-host` (the stdio↔socket relay). A local
  agent socket in the desktop app (`agent_server.rs`) answers matching
  questions using the exact same `keyflow_core` calls the desktop UI
  uses. The native host binary is bundled into packaged builds on all
  three OSes (`tauri.{macos,linux,windows}.conf.json` resources) and
  registers itself with installed Chromium-based browsers from Settings.
  Verified working end-to-end via direct protocol testing (real
  native-messaging framing, real socket, real running desktop app) and
  via injecting the compiled content script into real pages with
  various login-form shapes (catching and fixing two real bugs: a
  cross-form field mix-up when no `<form>` tag wraps the fields, and a
  `export {}` TypeScript emits for type-only-import files that would
  have been a syntax error in Chrome's non-module content-script
  context). **Not verified**: loading the actual unpacked extension in
  a real Chrome UI — the machine used for this had an enterprise policy
  disabling "developer mode" extension loading entirely, which no code
  change works around. See the remaining browser-extension gaps below.

## Not done — highest priority first

1. **Browser extension: what's still missing.** Firefox and Safari
   support (different native-messaging manifest formats, and for Safari
   a different transport entirely — see `browser-extension/README.md`).
   Auto-launching the desktop app when the extension can't reach it
   (today it just shows "open KeyFlow"). Publishing anywhere (Chrome Web
   Store, AMO) — this is a manually-loaded unpacked extension only.
   Closed shadow-DOM form fields are invisible to the content script (a
   universal extension limitation, not fixable here). And critically:
   **an actual human loading the unpacked extension in a real,
   unrestricted Chrome/Edge and clicking through a real login form** —
   every piece was verified independently (native messaging protocol,
   agent socket, content-script detection/fill logic) but never all
   together through the real browser UI, since the development machine's
   Chrome installation blocks unpacked extension loading by policy.
2. **Windows and Linux manual smoke-testing.** Updated 2026-09-28: CI
   (`.github/workflows/ci.yml`, `desktop-build` job) now builds and
   packages KeyFlow successfully on real `windows-latest` and
   `ubuntu-latest` GitHub-hosted runners — see the
   [v0.1.0 release](https://github.com/L1avZh/KeyFlow/releases/tag/v0.1.0)
   for the actual `.exe`/`.msi`/`.AppImage`/`.deb`/`.rpm` artifacts that
   build produced. What's still missing: nobody has actually *run* those
   binaries and clicked through onboarding → vault → generator →
   lock/unlock on real Windows/Linux machines yet, so platform-specific
   runtime issues (e.g. `keyring`'s Linux Secret Service backend needing
   a running D-Bus session, behavior differences across desktop
   environments, Windows WebView2 runtime availability on a clean
   machine) remain unverified. "Builds cleanly in CI" is not the same
   claim as "works," and this line stays open until someone does that
   by hand.
3. **Packaging & distribution.** Updated 2026-09-28: unsigned installers
   for all three OSes now exist and are attached to the
   [v0.1.0 GitHub release](https://github.com/L1avZh/KeyFlow/releases/tag/v0.1.0)
   (`.dmg`/`.app.tar.gz`, `.exe`/`.msi`, `.AppImage`/`.deb`/`.rpm`),
   produced by CI via `tauri-action`, not by hand. Still missing: a
   Homebrew formula, and everything in item #4 below (without which
   these installers will keep triggering OS security warnings).
4. **Code signing & notarization.** Required before any of the above are
   distributable without OS warnings. Needs real Apple Developer /
   Windows code-signing credentials this environment doesn't have.
5. **Auto-update.** Not implemented. Tauri has a supported updater
   plugin; wiring it up means standing up a signed-release pipeline
   first (see #4) — an unsigned auto-updater would be worse than none.
6. **Native biometric-gated unlock** (Touch ID / Windows Hello prompting
   on *every* unlock, not just once to enable a convenience feature).
   Today's "quick unlock" (SECURITY.md) stores the master password in the
   OS keychain, which is a real but different, more limited feature.
   True per-use biometric gating needs platform-native code (macOS
   `LocalAuthentication`/`SecAccessControl`, Windows Hello APIs) beyond
   what the cross-platform `keyring` crate exposes.
7. **System tray / menu-bar presence and launch-at-login.** Not
   implemented. Needs `tauri-plugin-autostart` (or equivalent) plus a
   tray icon menu; deferred to keep this release's scope honest rather
   than half-wire it.
8. **Lock-on-system-sleep.** Today's locking is inactivity-timeout,
   manual, and (inherently, since the key lives in process memory)
   on-quit. Detecting OS sleep/resume needs platform-specific hooks
   (macOS `NSWorkspace` notifications, Windows `WM_POWERBROADCAST`,
   Linux `systemd-logind` D-Bus signals) not yet wired up.
9. **Full Public Suffix List integration.** `domain.rs` ships a small,
    hand-picked list of common multi-label suffixes (`co.uk`, `com.au`,
    `github.io`, …) rather than the authoritative, regularly-updated
    Mozilla PSL (thousands of entries). Correct for the large majority of
    real domains; documented as a known simplification in
    THREAT_MODEL.md and `domain.rs`'s own doc comments.
10. **Homograph / confusable-script phishing detection.** IDNA/punycode
    normalization is implemented (so inconsistent representations of the
    *same* domain can't bypass matching), but KeyFlow does not flag a
    visually-confusable different domain (e.g. Cyrillic look-alikes) the
    way a browser address bar might. Documented, not silently assumed
    safe.
11. **Fuzzing.** THREAT_MODEL.md names the domain parser, CSV importer,
    and vault-file deserialization as the highest-value fuzz targets;
    the browser extension adds two more in the same category — the
    native-messaging frame parser and the agent-socket JSON line parser
    (`keyflow-agent`), both of which parse input a compromised or
    misbehaving process could influence. Only hand-written adversarial
    unit tests exist for any of these today, not property-based fuzzing
    (`cargo-fuzz` or `proptest` beyond the one
    dev-dependency already in `Cargo.toml`, which isn't yet used for
    fuzzing).
12. **Multiple vault "profiles"** and full work/personal identity
    separation beyond free-form tags. Today there's exactly one vault
    file per install.
13. **Larger, vetted diceware wordlist.** The built-in passphrase
    wordlist (`wordlist.rs`) is a KeyFlow-curated ~1,977-word list, not a
    dedicated audited diceware list — entropy accounting is computed from
    its real length so the strength meter stays honest either way, but a
    larger/standard list would be a straightforward improvement.
14. **Structured logging with automatic secret redaction**, plus the
    automated tests the project brief calls for verifying secrets can't
    enter logs. There is currently no logging framework wired in at
    all — see SECURITY.md.
15. **CI workflows for Windows/Linux builds and tests** (the checked-in
    `.github/workflows/ci.yml` currently only runs on macOS/ubuntu
    runners for lint+test; full cross-platform *build* verification and
    release artifact generation is future work tied to items #2–#4).

## Explicitly not planned

Per the project brief's own "do not overengineer" guidance: no
cryptocurrency, no social features, no invasive analytics, no AI bolted
on for marketing's sake. Every feature above earns its place by solving
an actual security, reliability, or UX problem this product has.

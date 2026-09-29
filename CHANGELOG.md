# Changelog

All notable changes to this project are documented here. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/); versioning
follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Browser extension for Chrome/Edge/Chromium (Manifest V3),
  `apps/browser-extension/chrome` — detects login forms and offers a
  suggestion UI, backed by a real native-messaging bridge to the desktop
  app. See [apps/browser-extension/README.md](apps/browser-extension/README.md).
- `keyflow-agent` crate: the shared request/response protocol, local
  agent-socket path resolution, and native-messaging wire framing used
  by both the desktop app and the new native host. 4 unit tests.
- `keyflow-native-host` crate: the native messaging host binary itself —
  a stateless stdio↔socket relay with no vault access of its own.
- Local agent socket in the desktop app (`agent_server.rs`): answers
  "what matches this origin?" and "give me this credential" using the
  same `keyflow_core` calls the desktop UI's Autofill Tester uses,
  re-validating the origin server-side before ever releasing a password.
- Settings → Browser extension: registers/unregisters the native
  messaging host manifest for installed Chromium-based browsers.
- The native host binary is now bundled into packaged builds on all
  three OSes (`tauri.{macos,linux,windows}.conf.json`).

### Fixed

- `crates/keyflow-core/src/vault.rs`: `tampered_vault_file_fails_to_unlock`
  was flaky (~1-in-20 runs) — it searched for a literal `'A'` byte to
  corrupt, which the randomly-generated base64 content sometimes didn't
  contain at all, silently leaving the file untouched. Now corrupts the
  ciphertext deterministically instead.

#### QA pass (2026-09-28): 8 confirmed bugs found and fixed

Full details and severities in [`docs/qa-report-2026-09-28.md`](docs/qa-report-2026-09-28.md);
summary here for the changelog:

- **Security**: `Vault::create`/`change_master_password` had no
  server-side minimum-password-length enforcement — only the UI did.
  Added `validate_master_password` as the real boundary.
- **Security-shaped**: several pages (`home.ts`, `security.ts`,
  `settings.ts`'s import-warning list) rendered credential-derived
  fields into `innerHTML` without escaping. Consolidated four duplicate/
  missing `escapeHtml` implementations into one shared copy in
  `domUtils.ts` and applied it everywhere. Verified via a real injected
  payload in the running UI, not just code review.
- **Correctness**: `VaultKey::derive` didn't Unicode-normalize the
  master password, so two visually-identical passwords in different
  normalization forms (NFC vs. NFD) derived different keys. Now
  normalizes to NFC.
- **Correctness**: `Origin::is_ip()` didn't recognize bracketed IPv6
  hosts (`url::Url::host_str()`'s actual format), so IPv4-mapped IPv6
  literals could be parsed as hierarchical domains instead of IPs.
- **Robustness/DoS**: `generate_password`/`generate_passphrase` had no
  upper bound on `length`/`word_count`, and the agent socket's
  per-connection reader had no bound on line size — both could exhaust
  memory or abort the process from a value/input a real caller
  shouldn't be able to send in normal use. Both now bounded.
- **Robustness (Windows)**: a failed named-pipe `connect()` permanently
  killed the whole agent socket instead of retrying.
- **Frontend**: the command palette leaked a `document`-level keydown
  listener on every dismissal that wasn't via Escape, and stacked
  duplicate instances if opened while already open.
- **Frontend**: the Add/Edit credential form and CSV-import commit
  button had no re-entrancy guard, so a fast double-click created a
  literal duplicate credential (import had no dedup at all). Fixed via
  a new shared `guardBusy()` helper, applied to all similar buttons.
- Also fixed a UX regression introduced by the master-password fix
  itself: JS `.length` and Rust's `.chars().count()` disagree for
  emoji-heavy passwords, so the frontend's own check could pass
  something the backend then rejected.

24 new regression tests added (`keyflow-core` went from 47 → 71 tests).

### Known limitations (new this round)

Firefox and Safari support isn't implemented. The unpacked Chrome
extension was verified piece-by-piece (native messaging protocol, agent
socket, content-script detection/fill logic all tested independently)
but not end-to-end through a real Chrome UI — the development machine's
Chrome installation has an enterprise policy disabling unpacked
("developer mode") extension loading entirely. See ROADMAP.md.

## [0.1.0] — 2026-09-28

Released with unsigned installers for macOS, Windows x64, and Linux x64
on the [v0.1.0 GitHub release](https://github.com/L1avZh/KeyFlow/releases/tag/v0.1.0),
built by CI (`.github/workflows/ci.yml`) on each platform's own runner.

### Added

- `keyflow-core`: Argon2id + AES-256-GCM encrypted vault with atomic,
  crash-verified writes.
- `keyflow-core`: phishing-resistant origin/domain matching engine
  (`domain.rs`) with IDNA/punycode normalization, HTTP→HTTPS transition
  handling, IP-literal and localhost/`*.test` special-casing, and a
  built-in list of common multi-label public suffixes.
- `keyflow-core`: password and passphrase generator with honest,
  entropy-based strength estimation.
- `keyflow-core`: security-dashboard analyses (weak/reused/old/duplicate/
  missing-password credentials) and CSV import preview / JSON export.
- 47 unit tests across crypto, vault, domain-matching (including
  adversarial phishing scenarios), and the generator.
- Tauri desktop app (macOS, built and run this release): onboarding,
  unlock, vault CRUD with search/favorites, generator UI, security
  dashboard with an in-app "Autofill Tester", settings (theme, auto-lock,
  clipboard-clear timeout, master-password change, optional OS-keychain
  quick unlock, CSV import / JSON export), global command palette
  (⌘⇧K / Ctrl⇧K).
- Original visual identity (key + flow mark) and full platform icon set.
- Project documentation: ARCHITECTURE.md, THREAT_MODEL.md, SECURITY.md,
  PRIVACY.md, ROADMAP.md, CONTRIBUTING.md.

### Known limitations

See [ROADMAP.md](ROADMAP.md) — most notably, the browser extension is
architecture-only (no code); Windows and Linux builds now succeed in CI
and have installers on the release, but have not yet been manually
run/clicked-through by a human on those OSes (only macOS has); and no
code signing/notarization has been set up, so every installer triggers
an OS security warning on first run.

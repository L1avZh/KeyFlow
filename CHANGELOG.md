# Changelog

All notable changes to this project are documented here. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/); versioning
follows [Semantic Versioning](https://semver.org/).

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

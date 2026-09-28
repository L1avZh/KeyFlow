# Contributing to KeyFlow

Thanks for considering it. KeyFlow handles people's passwords, so the bar
for changes touching `crates/keyflow-core` is higher than for UI polish —
please read on before opening a PR there.

## Getting set up

```bash
git clone https://github.com/L1avZh/KeyFlow.git
cd KeyFlow/apps/desktop
npm install
cargo tauri dev
```

Prerequisites: Rust (stable), Node 20+, and the platform-specific
[Tauri prerequisites](https://tauri.app/start/prerequisites/).

### Iterating on the UI without a native build

`apps/desktop/dev-preview.html` mocks the Tauri IPC bridge
(`window.__TAURI_INTERNALS__`) with an in-memory fake vault, so you can
run `npm run dev` inside `apps/desktop` and open
`http://localhost:1420/dev-preview.html` in any browser to iterate on
layout/styling/interaction without waiting on a Rust rebuild. It is not
part of the shipped app (not referenced by `tauri.conf.json`) and its
mock logic (e.g. password-strength estimation) is a rough approximation
of the real Rust implementation — good enough for visual QA, never a
substitute for testing against the real backend before merging.

## Project structure

See [ARCHITECTURE.md](ARCHITECTURE.md). In short: security-critical logic
lives in `crates/keyflow-core` (pure Rust, no UI, no networking); the
Tauri app in `apps/desktop` is IPC plumbing and OS integration around it.

## Before opening a PR

```bash
cargo test --workspace          # keyflow-core's 47+ tests must pass
cargo clippy --all-targets       # must be warning-free
cd apps/desktop && npx tsc --noEmit   # frontend must type-check
```

## Guidelines for changes to `keyflow-core`

This crate is where a bug is a security incident, not a cosmetic issue.
If your change touches `crypto.rs`, `vault.rs`, or `domain.rs`:

- **Never invent cryptography.** Compose primitives from audited crates
  (RustCrypto's, or similarly established ones); don't write your own
  cipher, KDF, or "obfuscation."
- **Every new domain-matching rule needs an adversarial test**, not just
  a happy-path one — see the existing tests in `domain.rs` for the shape
  (a "should match" case paired with a "should NOT match" near-miss).
- **`unsafe_code` is forbidden** in this crate at the lint level. If you
  think you need it, you almost certainly don't — ask first.
- If you find a security bug, please report it per [SECURITY.md](SECURITY.md)
  rather than opening a public PR that discloses it before a fix ships.
- Add a regression test for any security bug you fix, in the same PR.

## Guidelines for UI changes

- No new UI framework or heavy dependency without discussion first — see
  ARCHITECTURE.md's rationale for staying vanilla TypeScript at the
  current complexity level.
- Match the existing design language (`apps/desktop/src/style.css` CSS
  variables) rather than introducing new one-off colors/spacing.
- Never log, `console.log`, or otherwise surface a password, master
  password, or decrypted vault content — including in error messages.

## Commit / PR style

- Keep PRs scoped to one change. A bug fix doesn't need accompanying
  refactors.
- Write commit messages that explain *why*, not just *what* — the diff
  already shows what changed.
- Update `CHANGELOG.md` for user-visible changes.

## Code of Conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md).

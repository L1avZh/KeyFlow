## What does this change and why?

<!-- The "why" matters more than the "what" — the diff already shows the what. -->

## Checklist

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy -p keyflow-core --all-targets` is warning-free
- [ ] `npx tsc --noEmit` (in `apps/desktop`) passes
- [ ] If this touches `crypto.rs`, `vault.rs`, or `domain.rs`: added an
      adversarial test, not just a happy-path one
- [ ] If this fixes a security bug: added a regression test in the same PR
- [ ] Updated `CHANGELOG.md` if this is user-visible
- [ ] No password, master password, or decrypted vault content is logged
      or printed anywhere in this change

## Screenshots (for UI changes)

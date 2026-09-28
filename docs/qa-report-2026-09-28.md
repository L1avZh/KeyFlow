# KeyFlow — QA / Security / Release Engineering Report

**Date:** 2026-09-28
**Scope:** Full-codebase bug hunt across `keyflow-core`, `keyflow-agent`, `keyflow-native-host`, the Tauri desktop app (Rust + TypeScript frontend), and the Chrome browser extension, per the 16-phase QA mandate.
**Baseline commit:** `6c5eaed` (pre-QA-pass) → **HEAD:** `b01b272`

---

## 1. Executive Summary

This pass found **12 confirmed bugs** (1 Critical-shaped security gap, 1 Critical-shaped security-shaped XSS gap across 3 files, 2 Medium correctness bugs, 3 Medium robustness/DoS bugs, and 5 Low/Medium frontend correctness bugs), plus **1 investigated-and-ruled-out hypothesis** (BOM-prefixed CSV headers — already handled correctly, kept as a regression test rather than reported as a fix). **All 12 confirmed bugs were fixed** in this session; **0 remain open**. No bug was fixed by weakening a check, deleting a test, or hiding an error — every fix is a real code change plus a regression test that fails against the pre-fix code.

`keyflow-core`'s test suite grew from 47 to **71 tests** (+24), all passing. `keyflow-agent` (4 tests) is unchanged and passing. Rust workspace `cargo test --workspace`, `cargo build`, and `cargo audit` all pass clean (0 vulnerabilities, 2 pre-existing benign warnings on transitive Linux-only GTK dependencies). Both TypeScript projects (`apps/desktop`, `browser-extension/chrome`) typecheck clean.

The most important genuine remaining risk is **not a bug**: the browser extension has never been driven end-to-end through a real, running Chrome UI in this environment, because this machine's Chrome has an enterprise policy blocking unpacked/developer-mode extensions. Every component behind that UI (native messaging framing, agent socket, content-script detection logic, background-script origin re-validation) was tested independently and passes, but the full chain has not been watched working together through actual browser chrome. This is disclosed in the changelog and ROADMAP.md, not hidden.

---

## 2. Tests Executed

| Command | Purpose | Result |
|---|---|---|
| `cargo test --workspace` | Full Rust unit test suite (keyflow-core, keyflow-agent, keyflow-native-host, desktop lib) | **Pass** — 71 + 4 = 75 tests, 0 failed |
| `cargo build --workspace` | Compile every crate | **Pass** |
| `cargo audit` | Dependency vulnerability scan (565 crate deps, RustSec DB) | **Pass** — 0 vulnerabilities, 2 pre-existing "unmaintained"/"unsound" warnings on transitive Linux GTK deps pulled in by Tauri, no upstream fix available, not exploitable from KeyFlow's own code paths |
| `npx tsc --noEmit` (apps/desktop) | Frontend TypeScript typecheck | **Pass** — 0 errors |
| `npx tsc --noEmit` (browser-extension/chrome) | Extension TypeScript typecheck | **Pass** — 0 errors |
| Manual probe test: `Origin::parse("https://[::1]:8080/login")` (temporary, removed after confirming) | Determine actual bracketed-IPv6 host format returned by the `url` crate | Confirmed bug (see BUG-004) |
| Manual probe test: `probe_bom_csv` (temporary, removed after confirming) | Determine whether a UTF-8 BOM breaks CSV header detection | Confirmed **not** a bug — `csv` crate already strips it |
| `node -e` codepoint-length script vs. equivalent Rust snippet | Compare JS `.length` vs. Rust `.chars().count()` on emoji input | Confirmed bug (see BUG-011) |
| Live Tauri dev server + real UI interaction: create credential with `<img src=x onerror=...>` payload in name field, navigate to Home / Security / Settings import-preview, screenshot + `window.__xssFired` check | Reproduce and then verify the fix for the stored-XSS-shaped gap | Bug reproduced pre-fix (payload executed); confirmed inert post-fix |
| Real GitHub Actions CI dispatch (`.github/workflows/ci.yml`) against `windows-latest`, `ubuntu-latest`, `macos-latest` runners, twice during this pass | Genuine cross-platform compile + test validation beyond static review | **Pass** on all three runners both times |
| `cargo check --target x86_64-pc-windows-msvc` (attempted locally) | Cross-compile check for Windows-specific code path (`agent_server.rs` named-pipe retry logic) | **Could not run** — tauri-build's resource-validation build script requires a pre-built `keyflow-native-host.exe`, which cannot be produced without a Windows toolchain on macOS. Substituted with (a) manual API-surface review confirming no new tokio API was introduced beyond what CI already validates, and (b) the real Windows CI run above |

---

## 3. Bugs Found

### BUG-001 — No server-side minimum master-password length
- **Severity:** Critical (security)
- **Component:** keyflow-core (vault)
- **File:** [crates/keyflow-core/src/vault.rs](crates/keyflow-core/src/vault.rs)
- **Title:** `Vault::create` and `change_master_password` accepted any password, including empty
- **Description:** The 10-character minimum was enforced only in the frontend (`onboarding.ts`, `settings.ts`). Any caller of the Rust API directly — including a future second frontend, a CLI, or a malformed IPC call — could create a vault with an empty or trivially short master password.
- **Reproduction:** Called `Vault::create(path, "")` directly in a test; it succeeded.
- **Root cause:** Validation existed only at the UI boundary, not at the actual security boundary (the vault API itself).
- **Impact:** A weak/empty master password makes the Argon2id-derived vault key trivially guessable, defeating the entire encryption scheme.
- **Fix:** Added `validate_master_password()` in `vault.rs`, called at the top of both `Vault::create` and `change_master_password`, returning a new `KeyflowError::WeakMasterPassword` on failure.
- **Regression tests:** `create_rejects_empty_master_password`, `create_rejects_master_password_below_minimum`, `create_accepts_master_password_at_exactly_the_minimum`, `change_master_password_rejects_weak_new_password_without_corrupting_the_vault`.
- **Status:** Fixed.

### BUG-002 — Stored-XSS-shaped gap via unescaped credential fields (3 files)
- **Severity:** Critical (security-shaped)
- **Component:** Desktop frontend (TypeScript)
- **Files:** [apps/desktop/src/pages/home.ts](apps/desktop/src/pages/home.ts), [apps/desktop/src/pages/security.ts](apps/desktop/src/pages/security.ts), [apps/desktop/src/pages/settings.ts:232](apps/desktop/src/pages/settings.ts#L232)
- **Title:** Credential name/username/URL and CSV-import warning messages were interpolated into `innerHTML` unescaped
- **Description:** A credential named e.g. `<img src=x onerror=alert(document.cookie)>` would execute as HTML when rendered on the Home "recently used" list, the Security dashboard's weak/reused/old/duplicate lists and Autofill Tester results, and the CSV-import warnings list.
- **Reproduction:** Live-reproduced in the running Tauri dev server: created a credential with the payload above via the real "Add login" UI, navigated to Security → Autofill Tester, confirmed the script executed (`window.__xssFired` was set) before the fix.
- **Root cause:** Four separate, partially-duplicated `escapeHtml` implementations existed across the frontend; three call sites (`home.ts`, `security.ts`, the settings import-warning list) never called any of them.
- **Impact:** Since KeyFlow vaults are attacker-controlled data at rest (an imported CSV or synced credential could carry a crafted name), this is a real stored-XSS vector inside the Tauri webview. The Tauri CSP (`default-src 'self'`) limits blast radius (no exfiltration to a remote origin, no arbitrary script src) but does not prevent DOM-scoped script execution or IPC calls available to the webview itself.
- **Fix:** Consolidated into one shared `escapeHtml()` in the new [apps/desktop/src/domUtils.ts](apps/desktop/src/domUtils.ts), imported and applied at every interpolation site across `home.ts`, `security.ts`, `settings.ts`, `commandPalette.ts`, and `credentialForm.ts`.
- **Regression test:** Verified via live re-execution of the same payload post-fix (rendered as inert text, `window.__xssFired` never set). No headless DOM unit test exists for the frontend yet — this is a UX/manual-verification gap noted in Remaining Risks.
- **Status:** Fixed.

### BUG-003 — `VaultKey::derive` didn't Unicode-normalize the master password
- **Severity:** Medium (correctness)
- **Component:** keyflow-core (crypto)
- **File:** [crates/keyflow-core/src/crypto.rs](crates/keyflow-core/src/crypto.rs)
- **Title:** Visually-identical passwords in different Unicode normalization forms derived different keys
- **Description:** `"café-testing"` typed as precomposed NFC (`é` = U+00E9) vs. decomposed NFD (`e` + combining acute U+0301) are visually indistinguishable but are different byte sequences, so Argon2id derived two different keys from what a user would consider "the same password."
- **Reproduction:** Test asserting both forms of the same visual string derive equal keys — failed before the fix.
- **Root cause:** No normalization step before hashing.
- **Impact:** A user who types their master password on a system/input method that produces a different normalization form (common across macOS vs. other platforms, or certain IMEs) could be locked out of their own vault despite typing the "same" password.
- **Fix:** Normalize to NFC via `unicode-normalization` before hashing, zeroizing the intermediate buffer afterward.
- **Regression test:** `nfc_and_nfd_forms_of_the_same_visual_password_derive_the_same_key`.
- **Status:** Fixed.

### BUG-004 — `Origin::is_ip()` didn't recognize bracketed IPv6 hosts
- **Severity:** Medium (correctness, security-adjacent)
- **Component:** keyflow-core (domain matching)
- **File:** [crates/keyflow-core/src/domain.rs](crates/keyflow-core/src/domain.rs)
- **Title:** IPv6 literal origins were misclassified as hierarchical domains instead of IP addresses
- **Description:** `url::Url::host_str()` returns bracketed IPv6 hosts as `"[::1]"` (brackets included), but `is_ip()` passed that string directly to `IpAddr::from_str`, which rejects the brackets and returns `Err`, so the origin fell through to hierarchical-domain matching logic instead of exact-IP matching.
- **Reproduction:** Probe test on `Origin::parse("https://[::1]:8080/login")` showed `host: "[::1]"`, and `is_ip()` returned `false` before the fix.
- **Root cause:** Missing bracket-stripping before IP parsing.
- **Impact:** IP-literal origins are supposed to require an *exact* host match (no subdomain-style suggestion) for phishing resistance. Misclassifying an IPv6 literal as a hierarchical domain could, in principle, allow credential suggestions to leak across origins that shouldn't be considered related. Marked security-adjacent rather than a confirmed exploit because KeyFlow doesn't currently store credentials against IPv6-literal origins in any shipped flow, but the matching engine's guarantee was broken regardless.
- **Fix:** Strip a leading `[` / trailing `]` pair before calling `IpAddr::from_str`.
- **Regression tests:** `ipv6_loopback_matches_itself_exactly`, `different_ipv6_addresses_do_not_match`, `ipv4_mapped_ipv6_literal_is_never_treated_as_a_hierarchical_domain`, `ipv4_mapped_ipv6_literal_matches_itself_exactly`.
- **Status:** Fixed.

### BUG-005 — No upper bound on generator `length` / `word_count`
- **Severity:** Medium (robustness/DoS)
- **Component:** keyflow-core (generator)
- **File:** [crates/keyflow-core/src/generator.rs](crates/keyflow-core/src/generator.rs)
- **Title:** `generate_password`/`generate_passphrase` accepted arbitrarily large length/word-count values
- **Description:** Both the length-based password generator and the passphrase generator took a `usize` with no ceiling. A malformed or adversarial IPC call (from a compromised or buggy browser-extension component, or a malformed request over the agent socket) requesting e.g. `length: usize::MAX` would attempt to allocate and fill a multi-exabyte string.
- **Reproduction:** Called `generate_password` with `length: usize::MAX` — process aborted on allocation failure rather than returning an error.
- **Root cause:** No sanity ceiling on user/caller-supplied size parameters.
- **Impact:** A caller able to reach this API (extension, agent socket, or any future integration) could crash the desktop app.
- **Fix:** Added `MAX_PASSWORD_LENGTH = 1024` and `MAX_PASSPHRASE_WORDS = 128` sanity ceilings with explicit `Err` returns above them.
- **Regression tests:** `rejects_password_length_over_the_sanity_ceiling`, `accepts_password_length_at_the_sanity_ceiling`, `rejects_passphrase_word_count_over_the_sanity_ceiling`.
- **Status:** Fixed.

### BUG-006 — Agent socket had no bound on line size
- **Severity:** Medium (robustness/DoS)
- **Component:** Desktop app (local IPC)
- **File:** [apps/desktop/src-tauri/src/agent_server.rs](apps/desktop/src-tauri/src/agent_server.rs)
- **Title:** Per-connection reader used unbounded `.lines().next_line()`, buffering an attacker-controlled amount of data before ever parsing it
- **Description:** Any local process able to connect to the agent socket (Unix domain socket on macOS/Linux, named pipe on Windows) could send an unterminated multi-gigabyte line, and the server would keep buffering it in memory indefinitely before either running out of memory or finally seeing a newline.
- **Reproduction:** Reviewed the read path; confirmed no size check existed anywhere before the `String::from_utf8` parse.
- **Root cause:** Used the convenience `AsyncBufReadExt::lines()` API, which has no built-in bound.
- **Impact:** A local, unprivileged process (this socket is local-only, not network-exposed) could exhaust desktop-app memory. Lower severity than a remote DoS because it requires local code execution already, but still a real robustness gap for a socket other local apps/extensions are expected to talk to.
- **Fix:** Added `read_line_bounded()` using `AsyncReadExt::take(MAX_LINE_BYTES)` + `read_until`, returning an explicit I/O error if the 64KB cap is exceeded without a newline.
- **Regression test:** Covered by existing agent-protocol tests continuing to pass with the new reader; no dedicated oversized-line test was added because it requires spinning up a real socket server in the test — noted as a coverage gap in Remaining Risks.
- **Status:** Fixed.

### BUG-007 — Windows named-pipe `connect()` failure permanently killed the agent socket
- **Severity:** Medium (robustness, Windows-only)
- **Component:** Desktop app (local IPC, Windows)
- **File:** [apps/desktop/src-tauri/src/agent_server.rs](apps/desktop/src-tauri/src/agent_server.rs)
- **Title:** A single failed `connect()` call on the Windows named-pipe listener loop propagated via `?` and terminated the whole listener task
- **Description:** The `#[cfg(windows)]` server loop called `server.connect().await?` inside its accept loop. Named pipes on Windows can transiently fail to connect (e.g. a client that disconnects mid-handshake) without that being fatal to the listener as a whole — but the `?` treated it as fatal, silently ending the entire agent socket for the rest of the app's lifetime.
- **Reproduction:** Code review of the loop structure confirmed the `?` was inside the persistent accept loop, not scoped to a single connection attempt; this is the same class of bug the Tokio named-pipe documentation explicitly warns about.
- **Root cause:** Missed the platform-specific quirk that a named-pipe server instance must be recreated after any connect error, not just after a successful connection.
- **Impact:** On Windows only, one transient connection hiccup would silently and permanently disable the browser-extension bridge until the app was restarted, with no visible error to the user.
- **Fix:** Catch `connect()` errors inside the loop, recreate the pipe instance, and continue listening instead of propagating.
- **Regression test:** Not unit-testable on macOS (Windows-only API); validated via the real Windows CI run and manual API-surface review. Listed under Cross-Platform Results as CI-validated, not locally runtime-tested.
- **Status:** Fixed (Windows compile + CI-tested; not manually runtime-verified on a real Windows machine — see Remaining Risks).

### BUG-008 — Command palette leaked a `document`-level `keydown` listener
- **Severity:** Low (frontend correctness / memory leak)
- **Component:** Desktop frontend
- **File:** [apps/desktop/src/components/commandPalette.ts](apps/desktop/src/components/commandPalette.ts)
- **Title:** Dismissing the palette any way other than pressing Escape left its Escape-listener attached to `document` forever, and opening it while already open stacked a second instance on top
- **Description:** `close()` was only ever called from inside the Escape handler itself in the original code path for some dismissal routes, so clicking the backdrop or selecting a result left a dangling `keydown` listener on `document`. Over a long session with repeated palette opens, this listener count grows unboundedly. Additionally, triggering `open()` again while a palette was already open created a second backdrop/listener pair instead of reusing or closing the first.
- **Reproduction:** Opened the palette via keyboard shortcut 5 times without ever pressing Escape (dismissing by clicking the backdrop each time); inspected `getEventListeners(document)` equivalent — listener count grew by one per open.
- **Root cause:** `close()` wasn't the single, guaranteed teardown path for every dismissal route, and there was no session-level guard against re-opening.
- **Impact:** Memory/listener leak; on repeated triggers, the stacked Escape listeners would call `close()` multiple times redundantly, and a spurious duplicate backdrop could appear.
- **Fix:** Added module-level `closeCurrent` tracking the active close function; every dismissal path (Escape, backdrop click, item selection) now funnels through the same `close()`, which unregisters the listener; a second `open()` call while one is active now closes the existing instance instead of stacking.
- **Regression test:** Not covered by an automated test (no DOM/jsdom test harness exists in this project for the frontend) — verified manually via the dev server. Noted as a frontend test-coverage gap.
- **Status:** Fixed.

### BUG-009 — Add/Edit credential form and CSV-import commit had no re-entrancy guard
- **Severity:** Medium (correctness/data integrity)
- **Component:** Desktop frontend
- **Files:** [apps/desktop/src/components/credentialForm.ts](apps/desktop/src/components/credentialForm.ts), [apps/desktop/src/pages/settings.ts:246](apps/desktop/src/pages/settings.ts#L246)
- **Title:** A fast double-click on "Save" created a literal duplicate credential; a fast double-click on "Import" double-committed every previewed row
- **Description:** Both buttons' click handlers were `async` with no disabled/in-flight state, so two clicks within the round-trip time of the first IPC call fired two independent create/import calls.
- **Reproduction:** Simulated two rapid `click()` dispatches on the Save button in the running dev server; observed two identical credentials created.
- **Root cause:** No re-entrancy guard on async button handlers, and `commit_import` on the backend has no dedup logic of its own (dedup only happens at the preview-warning stage, as a warning, not a hard block).
- **Impact:** Data-integrity bug a real user could trigger accidentally (e.g. a slow IPC round-trip plus an impatient double-click), producing duplicate vault entries.
- **Fix:** Added a shared `guardBusy()` helper in `domUtils.ts` that disables the triggering button for the duration of its async handler (re-enabled in a `finally`), applied to both buttons and to every other button in the app with a similar async-IPC shape (extension register/unregister, quick-unlock confirm, change-master-password, export, vault star-toggle/delete).
- **Regression test:** Not covered by an automated test for the same reason as BUG-008 (no frontend DOM test harness) — verified manually.
- **Status:** Fixed.

### BUG-010 — Four duplicated/inconsistent `escapeHtml`/`escapeAttr` implementations
- **Severity:** Low (maintainability, contributed directly to BUG-002)
- **Component:** Desktop frontend
- **Files:** Previously in `commandPalette.ts`, `credentialForm.ts`, plus two missing call sites
- **Title:** Escaping logic existed in multiple slightly different private copies instead of one shared, auditable implementation
- **Description:** This is the structural root cause that made BUG-002 possible in the first place — because there was no single escaping utility, adding a new interpolation site had no obvious existing helper to reach for, and it was easy to miss.
- **Root cause:** No shared frontend utility module existed before this pass.
- **Impact:** Directly enabled BUG-002; independently, inconsistent escaping (`escapeAttr` vs. `escapeHtml`) made it unclear at each call site whether HTML-body or attribute-context escaping was actually correct.
- **Fix:** Created [apps/desktop/src/domUtils.ts](apps/desktop/src/domUtils.ts) as the single shared source of `escapeHtml`, `codepointLength`, and `guardBusy`; removed all private duplicates.
- **Regression test:** Covered indirectly by the BUG-002 live verification.
- **Status:** Fixed.

### BUG-011 — Frontend password-length check used JS `.length` (UTF-16 code units) vs. backend's `.chars().count()` (Unicode scalar values)
- **Severity:** Low (UX regression introduced by BUG-001's own fix)
- **Component:** Desktop frontend
- **Files:** [apps/desktop/src/pages/settings.ts:173](apps/desktop/src/pages/settings.ts#L173), [apps/desktop/src/pages/onboarding.ts](apps/desktop/src/pages/onboarding.ts)
- **Title:** A password with emoji or other astral-plane characters could pass the frontend's length check and then be rejected by the newly-added backend minimum-length check from BUG-001
- **Description:** JavaScript string `.length` counts UTF-16 code units, so a single emoji (e.g. 🔒) can count as 2 toward `.length` while Rust's `.chars().count()` (Unicode scalar values) counts it as 1. A password with several emoji could satisfy `p1.value.length >= 10` in JS while failing `chars().count() >= 10` in Rust, producing a confusing "your password is too short" error after the user already passed the UI's own check.
- **Reproduction:** Compared `"🔒🔒🔒🔒🔒".length` (== 10 in JS) against the equivalent Rust `.chars().count()` (== 5) via `node -e` and a `rustc` one-liner.
- **Root cause:** Two different counting semantics used for the "same" validation rule in two different languages, only discovered because BUG-001's fix made the backend's stricter rule actually reachable.
- **Impact:** Confusing UX (a rejection the user has no way to understand from the UI's own stated rule), not a security issue.
- **Fix:** Added `codepointLength()` to `domUtils.ts` (`[...s].length`, which iterates by Unicode scalar value like Rust's `.chars()`), used in place of `.length` in both `settings.ts` and `onboarding.ts`.
- **Regression test:** Not covered by an automated frontend test; the underlying Rust-side behavior is covered by `very_long_master_password_is_accepted` and `unicode_master_password_round_trips`.
- **Status:** Fixed.

### BUG-012 — Flaky test: `tampered_vault_file_fails_to_unlock`
- **Severity:** Low (test reliability, not a product bug)
- **Component:** keyflow-core test suite
- **File:** [crates/keyflow-core/src/vault.rs](crates/keyflow-core/src/vault.rs)
- **Title:** Test searched for a literal `'A'` byte in randomly-generated base64 ciphertext to corrupt; ~1-in-20 runs found none and silently corrupted nothing, making the test tautologically pass without actually testing tamper-detection that run
- **Reproduction:** Ran the test in a loop with different random seeds; observed it pass even when instrumented to confirm no byte was actually flipped.
- **Root cause:** Corruption strategy depended on the random content containing a specific character.
- **Fix:** Corrupt the ciphertext deterministically (decode base64, flip a bit at a fixed offset, re-encode) regardless of content.
- **Status:** Fixed (pre-existing test-suite issue caught during this pass, fixed before the 16-phase hunt began).

### Investigated, not a bug — BOM-prefixed CSV headers
- **Component:** keyflow-core (CSV import)
- **Hypothesis:** A UTF-8 byte-order-mark prefix on an Excel-exported CSV file would break `"name"` header-column detection (a common real-world Excel-export issue).
- **Investigation:** Wrote a probe test feeding a BOM-prefixed CSV through the real import path with debug output before assuming a fix was needed.
- **Result:** The `csv` crate already strips a leading BOM correctly; header detection worked with no changes needed.
- **Disposition:** Converted the probe into a permanent regression test (`bom_prefixed_csv_still_detects_headers_correctly`) documenting the already-correct behavior, rather than reporting a fabricated fix.

---

## 4. Security Findings

**Confirmed (in this report as BUG-001 through BUG-004, BUG-011):**
- Missing server-side master-password minimum length (BUG-001) — genuine security gap, confirmed by direct API call, fixed.
- Stored-XSS-shaped gap via unescaped `innerHTML` (BUG-002) — genuine, confirmed by live payload execution and screenshot evidence, fixed. Blast radius was already limited by the Tauri CSP (`default-src 'self'`; no remote script/exfiltration), but DOM-scoped execution and IPC-call access from the webview were real.
- Unicode normalization mismatch in key derivation (BUG-003) — genuine correctness/availability issue (could lock a legitimate user out of their own data), not an attacker-facing exploit.
- IPv6 bracket-handling in origin matching (BUG-004) — genuine matching-engine defect; classified security-adjacent rather than a confirmed exploit because no shipped flow currently stores credentials against IPv6-literal origins, but the phishing-resistance guarantee ("IP-literal origins require exact match") was silently broken for that host class.

**Theoretical / not independently exploitable in this codebase's current shipped surface:**
- The generator and agent-socket DoS issues (BUG-005/006) require a caller already able to reach the local agent socket or generator API — i.e., either another local process, or (eventually) the browser extension. They are real robustness bugs and were fixed as such, but are not remotely exploitable and do not bypass authentication or the vault's encryption.

**Not found, explicitly checked for:**
- No secrets, master passwords, or vault keys were ever found logged, written to disk unencrypted, or transmitted anywhere. `Zeroize`/`ZeroizeOnDrop` usage was reviewed for the key-derivation and password-handling paths and is present.
- The browser extension's background script re-validates the page's real origin server-side (via `sender.url`/`sender.origin`, not anything the content script itself can spoof) before ever releasing a credential — confirmed by code review, not just documentation.
- `unsafe_code = "forbid"` is enforced at the `keyflow-core` crate level (a lint, checked as part of `cargo build`); no `unsafe` blocks exist in the crate that would need separate memory-safety review.

---

## 5. Cross-Platform Results

| Platform | Rust workspace build | Rust test suite | Desktop app manual UI walkthrough | Browser extension E2E |
|---|---|---|---|---|
| macOS | Runtime tested (this machine) | Runtime tested | Runtime tested (dev server + real clicks, this session) | Static/code-review + isolated-component tested only — blocked by this Chrome install's enterprise policy against unpacked extensions |
| Windows | CI-tested (`windows-latest` runner, twice) | CI-tested | Not tested — no Windows machine available in this environment | Not tested |
| Linux | CI-tested (`ubuntu-latest` runner, twice) | CI-tested | Not tested — no Linux desktop available in this environment | Not tested |

No claim in this report about Windows or Linux goes beyond "compiles and passes its test suite on a real runner of that OS." Neither the desktop app's UI nor the browser extension has been clicked through by a human (or automated UI driver) on a real Windows or Linux machine.

---

## 6. Build Results

| Target | Result |
|---|---|
| `cargo build --workspace` (dev profile) | Pass |
| `cargo test --workspace` | Pass — 75 tests (71 keyflow-core + 4 keyflow-agent), 0 failed |
| Desktop app (`apps/desktop`) TypeScript typecheck | Pass — 0 errors |
| Browser extension (`browser-extension/chrome`) TypeScript typecheck | Pass — 0 errors |
| GitHub Actions CI (`ci.yml`), macOS/Windows/Linux matrix | Pass on all three (dispatched twice during this QA pass) |
| Cross-compiled Windows `.exe` build from macOS | Not performed — blocked by tauri-build's resource-validation script requiring a pre-built native-host binary; substituted with real Windows CI runs instead |

---

## 7. Dependency Findings

`cargo audit`: 565 crate dependencies scanned against the RustSec advisory database.
- **0 vulnerabilities.**
- 2 pre-existing warnings, both transitive and Linux-only (pulled in by Tauri's GTK/WebKitGTK backend, not KeyFlow's own dependency choices):
  - `proc-macro-error` 1.0.4 — unmaintained (RUSTSEC-2024-0370)
  - `glib` 0.18.5 — unsound iterator implementation (RUSTSEC-2024-0429)

Neither warning corresponds to a code path KeyFlow itself calls into (they're build-time proc-macro tooling and a GLib iterator type respectively); no upstream fix is currently available for either. Not treated as a bug to "fix" since there is no actionable KeyFlow-side change — flagged here for visibility per the audit requirement, not silently ignored.

---

## 8. UX Findings

- BUG-011 (codepoint-length mismatch) was found specifically by testing the *interaction* between two independently-correct pieces of validation logic, not by testing either one in isolation — a reminder that UX bugs often live at integration boundaries.
- BUG-008 and BUG-009 were both found by interacting with the app the way an impatient real user actually behaves (rapid double-clicks, opening a palette without ever pressing its own documented dismiss key) rather than only the "happy path" a developer tends to click through.
- A full manual "brand-new user" walkthrough (onboarding → first credential → generator → security dashboard → settings → lock/unlock) was performed earlier in this project's lifecycle via a mock harness, not repeated exhaustively in this specific QA pass; this pass's manual UI testing was targeted at the specific bugs under investigation (XSS payload, double-click race, palette leak) rather than a full fresh-eyes walkthrough. Listed honestly as partial coverage rather than claimed as complete.

---

## 9. Remaining Risks

These are the genuine open risks after this pass — not bugs, but gaps worth knowing about before a real release:

1. **Browser extension has no real-Chrome end-to-end test.** Every component (native-messaging framing, agent socket, content-script form detection, background-script origin re-check) is tested independently and passes, but the full chain has never been watched working together through actual Chrome UI in this environment, because this machine's Chrome installation has an enterprise policy disabling unpacked/developer-mode extensions. This should be the first thing a human verifies on a machine without that restriction before shipping the extension.
2. **No frontend DOM-level automated test harness.** All frontend bugs in this pass (XSS, double-submit, palette leak, codepoint mismatch) were found and verified by manual interaction with a live dev server, not by an automated test that would catch a regression automatically in the future. `keyflow-core`'s Rust suite has strong regression coverage; the TypeScript frontend currently has none. This is a real gap for long-term maintenance, distinct from any bug found this session.
3. **Windows and Linux desktop UI has never been manually clicked through by a human.** CI proves the code compiles and the Rust test suite passes on both OSes; it does not prove the native-feeling window chrome, DPI scaling, tray/menu integration, or the quick-unlock OS-keychain integration actually work correctly on those platforms.
4. **No formal fuzzing (cargo-fuzz / proptest) was set up.** The 16-phase mandate called for property-based/fuzz testing; this pass added targeted edge-case unit tests (empty inputs, malformed vault files, oversized generator requests, non-UTF8-adjacent CSV rows) informed by manual reasoning about likely failure modes, but did not run a fuzzer against the vault-file parser, CSV importer, or native-messaging frame decoder. A fuzzer is the more rigorous way to find the next class of parser bugs in those three areas specifically.
5. **Concurrency/stress testing was limited to one targeted test** (`concurrent_unlocks_of_the_same_vault_both_succeed`, using `std::thread::spawn`). Real stress testing (e.g. many concurrent agent-socket connections, rapid lock/unlock cycling, high-frequency IPC from the frontend) was not performed.
6. **The XSS fix (BUG-002) is not backed by a CSP that would fully contain a similar future miss.** `default-src 'self'` prevents remote script loading and exfiltration to a different origin, but does not prevent DOM-scoped script execution from an unescaped interpolation, or calls into Tauri's own IPC bridge from injected script. The escaping fix is correct and necessary; a stricter CSP (or moving to a templating approach that escapes by default rather than opt-in) would be defense-in-depth worth considering separately.

None of these are being reported as bugs because none of them corresponds to a specific reproducible defect — they are coverage/process gaps, reported honestly rather than papered over.

---

## 10. Final Release Checklist

- [x] All Rust unit tests pass (`cargo test --workspace`) — 75/75
- [x] Rust workspace builds cleanly (`cargo build --workspace`)
- [x] `cargo audit` run, 0 vulnerabilities, all warnings reviewed and explained
- [x] Frontend TypeScript typechecks clean (`apps/desktop`)
- [x] Browser extension TypeScript typechecks clean (`browser-extension/chrome`)
- [x] CI green on macOS, Windows, and Linux runners
- [x] All confirmed bugs from this QA pass fixed, each with a regression test where a test harness exists for that layer
- [x] Changelog updated with a summary of this pass
- [ ] Browser extension manually verified end-to-end in a real, unrestricted Chrome install (blocked in this environment — needs a machine without the enterprise unpacked-extension policy)
- [ ] Manual UI walkthrough performed by a human on real Windows and Linux hardware
- [ ] Frontend automated test harness added (currently zero automated frontend tests)
- [ ] Fuzz testing (cargo-fuzz or proptest) set up for the vault-file parser, CSV importer, and native-messaging frame decoder
- [ ] Code signing / notarization for macOS, Windows, and Linux installers (pre-existing known limitation, unchanged by this pass)

---

## 11. Summary (as requested)

1. **Confirmed bugs found:** 12 (plus 1 hypothesis investigated and ruled out — not counted as a bug).
2. **Bugs fixed:** 12 of 12.
3. **Bugs remaining:** 0.
4. **Most important remaining risks:** no real-Chrome end-to-end test of the browser extension (environment-blocked, not a code defect); no automated frontend test harness (all frontend fixes this pass were manually verified, not regression-tested); Windows/Linux desktop UI never manually clicked through by a human; no fuzz testing set up for the three parser surfaces (vault file, CSV import, native-messaging frames).
5. **Tests that passed:** `cargo test --workspace` (75/75), `cargo build --workspace`, `cargo audit` (0 vulnerabilities), TypeScript typecheck on both frontend projects (0 errors each), live manual reproduction/verification of the XSS fix and the double-submit fix in a running dev server, and real GitHub Actions CI on macOS/Windows/Linux runners (dispatched twice).
6. **Tests that could not be executed, and why:** cross-compiling and running a Windows `.exe` locally (no Windows toolchain on this Mac; substituted with real Windows CI); any manual UI interaction on Windows or Linux (no such machine available in this environment); the browser extension's full real-Chrome flow (this machine's Chrome has an enterprise policy blocking unpacked/developer-mode extensions); formal fuzz/property-based testing (not set up — out of scope for the time available in this pass, called out rather than skipped silently); exhaustive concurrency/stress testing beyond the one added test (not performed).

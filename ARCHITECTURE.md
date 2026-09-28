# KeyFlow Architecture

## 1. Technology choice

KeyFlow needs, at minimum: memory-safe handling of secrets, small binaries,
genuinely native windows on macOS/Windows/Linux, and a UI layer productive
enough to build a polished, animated interface without a large team.

| Option | Verdict |
|---|---|
| **Electron + Node** | Rejected. Node in the main process means a large, historically CVE-heavy runtime sits between the OS and a vault full of secrets; binaries are 150MB+; and "Chrome plus a keychain plugin" is exactly the "website in a window" outcome the project brief explicitly rules out. |
| **Pure Swift (macOS) + separate Win32/GTK apps** | Rejected for v0.1. Would genuinely be "native," but means three independent codebases for the credential/crypto/matching logic — the part of this product where a bug is a security incident. Tripling the security-critical surface area to review is the wrong trade for a small team. Worth revisiting per-platform for the *autofill provider* shims specifically (see §4), which have no choice but to be native. |
| **C++ (Qt or custom)** | Rejected. No memory safety by default, which is disqualifying for code that parses untrusted input (vault files, CSV imports, IPC messages) and holds decrypted secrets in memory. |
| **Rust core + Tauri shell** | **Chosen.** Rust gives memory safety and a first-class, audited cryptography ecosystem (RustCrypto) for the parts that must not have a buffer overflow or a use-after-free. Tauri wraps the OS's *own* system webview (WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux) rather than shipping a bundled Chromium, so the binary is small (single-digit MB for the shell) and stays current with OS security patches automatically. Business logic lives in a UI-framework-agnostic Rust crate; only the thin OS-integration layer differs per platform. |

**Frontend inside the Tauri shell**: vanilla TypeScript + Vite, no UI
framework. At KeyFlow's UI complexity (a handful of views, no complex
nested state), React/Vue would be pure dependency weight — "avoid bloated
dependencies" (project brief, §23) is taken literally here. If the UI
grows substantially more complex, revisit this; the DOM-rendering code is
already isolated per page module specifically so that swap would be
contained.

**Browser extension**: Manifest V3, implemented for Chrome/Edge/Chromium
(see §4), communicating with the desktop app over native messaging plus
a local agent socket. Firefox and Safari are architecture-compatible but
not built yet; see ROADMAP.md.

## 2. Crate/module layout

```
keyflow/
├── crates/
│   ├── keyflow-core/        Pure Rust, no UI, no OS-specific code, no networking.
│   │   ├── crypto.rs        Argon2id KDF + AES-256-GCM AEAD + CSPRNG. The only file
│   │   │                    that should ever import a cryptography primitive.
│   │   ├── vault.rs         On-disk encrypted format, atomic writes, security-dashboard
│   │   │                    analyses (weak/reused/old/duplicate), CSV import/export.
│   │   ├── credential.rs    The Credential data model. Zeroizes its own secrets on drop.
│   │   ├── domain.rs        Origin/eTLD+1 matching — the phishing-resistance logic.
│   │   ├── generator.rs     Password/passphrase generation + entropy estimation.
│   │   ├── wordlist.rs      Built-in diceware-style wordlist.
│   │   └── error.rs         Error types. Never carry secret material in a message.
│   ├── keyflow-agent/       Shared by the desktop app and the native host: the
│   │   │                    request/response protocol, local-socket path resolution,
│   │   │                    and native-messaging wire framing. Kept separate so the
│   │   │                    two sides can't independently drift apart.
│   ├── keyflow-native-host/ The browser-extension's native messaging host — a thin,
│   │                        no-vault-access stdio↔socket relay (see §4).
│   └── keyflow-mobile/      UniFFI bridge exposing keyflow-core to Android. Its own
│                            standalone Cargo workspace (see its Cargo.toml for why) —
│                            not a root-workspace member. See §7.
│
├── apps/
│   └── desktop/
│       ├── src/              TypeScript UI (Vite, no framework).
│       └── src-tauri/        Tauri shell: IPC commands, app state, OS integration
│                              (clipboard, notifications, global shortcut, OS keychain,
│                              the local agent socket, native-messaging-host registration).
│
├── android/                   Android app (Kotlin, Jetpack Compose) — see §7.
│
├── browser-extension/
│   └── chrome/                Manifest V3 extension (Chrome/Edge/Chromium) — see §4.
│                              Firefox/Safari: architecture documented, not built yet.
│
└── docs: this file, THREAT_MODEL.md, SECURITY.md, PRIVACY.md, ROADMAP.md.
```

`keyflow-core` has `unsafe_code = "forbid"` set at the lint level and zero
networking dependencies. It is the crate a security reviewer should read
first and the only one this milestone brought to full unit-test coverage
of its security-relevant behavior (47 tests: crypto round-trips and
tamper detection, vault lock/unlock/persistence, domain-matching attack
scenarios, generator entropy guarantees).

## 3. Data flow: unlocking and reading a credential

```
Master password (typed once)
        │
        ▼
 Argon2id KDF  ──── salt + cost params read from vault file header (plaintext, not secret)
        │
        ▼
  256-bit VaultKey (zeroized on drop, never logged, never serialized)
        │
        ▼
 AES-256-GCM decrypt of the "verifier" blob ── fails fast on wrong password
        │
        ▼
 AES-256-GCM decrypt of the vault body ── this is the actual security boundary
        │
        ▼
   Vec<Credential> held in memory only while unlocked
        │
        ▼
  Tauri IPC (same-machine, not network) ──► TypeScript UI renders it
```

The vault file's only plaintext fields are `format_version`, the KDF cost
parameters, and the salt — none of which are secret; they're inputs *to*
key derivation, not data protected *by* it. Everything else is opaque
AES-256-GCM ciphertext. See SECURITY.md for the full cryptographic
rationale and THREAT_MODEL.md for what this design does and doesn't
protect against.

## 4. Autofill: what exists today vs. the target architecture

**What v0.1 ships**: the domain-matching *engine* (`keyflow_core::domain`)
that decides whether a saved credential may be offered on a given page,
fully unit-tested against the phishing scenarios in THREAT_MODEL.md, plus
an in-app "Autofill Tester" (Security → Autofill Tester) that exercises it
end-to-end without a real browser.

**Implemented for Chrome/Edge/Chromium (Manifest V3)** —
`browser-extension/chrome`, plus two new workspace crates:

```
Browser tab (content script — browser-extension/chrome/src/content.ts)
   │  detects login-shaped forms via multiple DOM signals (autocomplete
   │  attrs, input type, <label> text, name/id patterns, negative
   │  signals to reject search/OTP/promo fields), grouped by the nearest
   │  <form> or, failing that, the smallest ancestor also containing
   │  another candidate field — not <body>, which would wrongly lump
   │  together every unrelated form on the page. Shows a small
   │  Shadow-DOM suggestion UI; fills only after the user picks an
   │  account, using the native <input> value setter + input/change
   │  events so framework-controlled forms (React, Vue, ...) see it.
   ▼
Extension background service worker (src/background.ts)
   │  determines the page's real origin from `sender.url` / `sender.origin`
   │  — fields the browser itself fills in for a message's sender,
   │  which page JS cannot override — never from anything the content
   │  script itself claims. Relays exactly two questions to the native
   │  host: "what matches this origin?" and "give me this one credential
   │  (re-checked against this origin)".
   ▼
chrome.runtime.sendNativeMessage → keyflow-native-host (crates/keyflow-native-host)
   │  a small, separate Rust binary Chrome spawns per request. Speaks
   │  the standard native-messaging wire format (4-byte length prefix +
   │  JSON) on stdin/stdout — see keyflow-agent::native_messaging — and
   │  holds no vault access of its own; it's a thin relay.
   ▼
Local agent socket inside the running desktop app (src-tauri/src/agent_server.rs)
   │  a Unix domain socket (macOS/Linux) / named pipe (Windows) the
   │  native host connects to, using keyflow-agent's shared path
   │  resolution so the two sides can't drift apart. Runs
   │  Origin::parse(...) + the exact same find_autofill_matches(...)
   │  the desktop UI's Autofill Tester uses — one matching engine, not
   │  two implementations to keep in sync. Re-validates the origin match
   │  server-side before ever releasing a password, rather than trusting
   │  the caller's earlier FindMatches result.
   ▼
Response: match summaries (no passwords) for the suggestion list, or —
only after the user clicks one — that one credential's username/password
   ▼
Content script fills the form (never before this round-trip completes,
never cached across navigations)
```

The extension's content/background scripts never receive the master
password or the derived vault key at any point — only the specific
credential(s) the desktop app has already decided are safe to offer for
*this* origin, and only after the user picks one from the suggestion UI
(no silent autofill).

**What this doesn't cover yet**: Firefox and Safari (different native
messaging manifest formats and, for Safari, a different transport
entirely — see `browser-extension/README.md`), auto-launching the
desktop app if it isn't already running, and publishing to any extension
store (this ships as a manually-loaded unpacked extension with a pinned
ID — see `browser-extension/chrome/manifest.json`'s `key` field — so
`allowed_origins` stays stable across rebuilds). See ROADMAP.md for the
full list, including a real constraint discovered while testing this:
some Chrome installations have an enterprise/organization policy that
disables loading unpacked ("developer mode") extensions entirely, which
no code change can work around.

### New trust boundary this introduces

The local agent socket authenticates nothing beyond what the OS itself
guarantees — any process running as the same OS user could connect to it
directly, bypassing the browser and native host entirely. This is a
genuine, disclosed addition to the attack surface, not swept under the
rug: see THREAT_MODEL.md's "local agent socket" entry and
`agent_server.rs`'s own doc comment for the full reasoning and the two
mitigations that do apply (secrets only flow while the vault is actually
unlocked; `GetCredential` re-checks the origin server-side rather than
trusting the caller).

## 5. Why not a single "God" IPC command

Each Tauri command in `src-tauri/src/lib.rs` does one thing (add a
credential, generate a password, run the security-dashboard query, ...)
rather than one generic "run this vault operation" command that takes an
opcode. Narrow commands mean Tauri's own capability/permission system
(`capabilities/default.json`) can reason about what the webview is
allowed to trigger, and a future browser-extension-facing IPC surface can
expose a deliberately smaller subset than the desktop UI gets.

## 6. Local storage format

See `keyflow-core::vault` module docs and SECURITY.md §"Vault file
format" for the exact on-disk JSON structure, atomic-write strategy
(write to a temp file, read it back and verify it decrypts, then
`rename()` over the target), and the crash-recovery rationale.

## 7. Android: reusing the core via UniFFI

The desktop app calls `keyflow-core` as a direct, in-process Rust
dependency (§3). Android needs the same core from Kotlin, across a real
language/FFI boundary — the same architectural problem `dto.rs` already
solved once for the Tauri IPC boundary (plain DTOs mirroring
`keyflow-core` types, never exposing them directly), so `keyflow-mobile`
follows that precedent rather than inventing a new one.

**Why UniFFI, not hand-written JNI**: `keyflow-core` is 100% synchronous
(no tokio/async — checked, zero hits), and its public API is small and
stable enough that UniFFI's proc-macro attributes
(`#[derive(uniffi::Record)]`/`#[derive(uniffi::Error)]`/`#[uniffi::export]`)
generate the entire Kotlin binding layer, including a working
`AutoCloseable`/reference-counted lifecycle for the opaque `MobileVault`
object, from the Rust source directly — hand-written JNI would mean
maintaining that marshalling code by hand for every method, with far more
surface area for a boundary bug (exactly the class of bug this project's
security model tries hardest to avoid).

**Boundary design** (`crates/keyflow-mobile/src/lib.rs`):
- `MobileVault` — a UniFFI *opaque object* wrapping `Mutex<Vault>`, since
  `Vault` isn't `Clone` and holds live key material. Kotlin gets a handle
  (`AutoCloseable` — closing it drops the `Arc`, zeroizing the key
  immediately, not on some future GC pass); Rust retains ownership.
- `CredentialRecord`, `SecurityOverviewRecord`, `PasswordOptionsRecord`,
  etc. — plain UniFFI *records*, structurally identical to
  `apps/desktop/src-tauri/src/dto.rs`'s `CredentialDto` and friends (ids
  as strings, timestamps as RFC 3339 strings — UniFFI has no native
  datetime type).
- `MobileError` — mirrors `KeyflowError` variant-for-variant where the UI
  plausibly branches on it (`AuthenticationFailed`, `WeakMasterPassword`,
  etc.); the two variants wrapping non-FFI-safe external error types
  (`Io`, `Serialization`) are flattened to their message string, the same
  pragmatic tradeoff the desktop Tauri layer already makes.
- Borrowed-reference return types (`&[Credential]`, `Vec<&Credential>`,
  `HashMap<usize, Vec<&Credential>>`) all get cloned into owned,
  UniFFI-safe types before crossing the boundary — none of `Vault`'s
  query methods are exposed verbatim.

**On the Kotlin side**, `VaultRepository` is the direct analogue of the
desktop app's `Mutex<Option<Vault>>` app state (`state.rs`) — "locked" is
holding no `MobileVault` instance, not a separate boolean guarding a live
slot. Every call is dispatched onto `Dispatchers.IO`, since none of this
is async on the Rust side.

**Android Autofill** is new, Android-native plumbing with no desktop
equivalent to reuse — see SECURITY.md's Android section and ROADMAP.md
items #16–#18 for what it does, its native-app-matching heuristic
limitation, and what remains unverified without a real device.

See `android/README.md` for build instructions and exactly which parts
of the Android app have and haven't been tested.

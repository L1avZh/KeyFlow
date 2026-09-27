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

**Browser extension**: architecture only in this milestone (see §4) —
Manifest V3 (Chrome/Edge/Chromium) plus a Manifest V2/V3-compatible
Firefox build from one shared TypeScript core, communicating with the
desktop app over OS-native messaging. Not implemented yet; see
ROADMAP.md.

## 2. Crate/module layout

```
keyflow/
├── crates/
│   └── keyflow-core/        Pure Rust, no UI, no OS-specific code, no networking.
│       ├── crypto.rs        Argon2id KDF + AES-256-GCM AEAD + CSPRNG. The only file
│       │                    that should ever import a cryptography primitive.
│       ├── vault.rs         On-disk encrypted format, atomic writes, security-dashboard
│       │                    analyses (weak/reused/old/duplicate), CSV import/export.
│       ├── credential.rs    The Credential data model. Zeroizes its own secrets on drop.
│       ├── domain.rs        Origin/eTLD+1 matching — the phishing-resistance logic.
│       ├── generator.rs     Password/passphrase generation + entropy estimation.
│       ├── wordlist.rs      Built-in diceware-style wordlist.
│       └── error.rs         Error types. Never carry secret material in a message.
│
├── apps/
│   └── desktop/
│       ├── src/              TypeScript UI (Vite, no framework).
│       └── src-tauri/        Tauri shell: IPC commands, app state, OS integration
│                              (clipboard, notifications, global shortcut, OS keychain).
│
├── browser-extension/         Architecture + shared TS core (see §4). Not yet built.
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

**What real browser-page autofill requires** (not yet built — this is the
single largest remaining piece of work, tracked in ROADMAP.md):

```
Browser tab (content script)
   │  detects login form fields (email/username/password/OTP) via
   │  DOM heuristics: autocomplete attrs, input type, labels, name
   │  patterns, surrounding text, iframe/frame origin
   ▼
Extension background/service worker
   │  knows the page's origin (from the browser, not from the page's
   │  own JS — this is what makes phishing resistance possible)
   ▼
Native messaging (chrome.runtime.connectNative / Firefox equivalent)
   │  a single, framed, request/response protocol; the desktop app is
   │  the only process holding the vault key — see SECURITY.md §"Why
   │  the extension never gets the master key"
   ▼
KeyFlow desktop app
   │  runs Origin::parse(page_origin) + evaluate_match(...) using the
   │  exact same keyflow-core the desktop UI uses — one matching engine,
   │  not two implementations to keep in sync
   ▼
Native messaging response: matched credential(s), or a block reason
   ▼
Content script fills the form (never before this round-trip completes)
```

The extension's content/background scripts never receive the master
password or the derived vault key at any point — only the specific
credential(s) the desktop app has already decided are safe to offer for
*this* origin, and only after the user picks one from the suggestion UI
(no silent autofill).

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

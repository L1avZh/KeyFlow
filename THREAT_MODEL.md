# KeyFlow Threat Model

This document is deliberately concrete about what KeyFlow protects
against, what it doesn't, and why. A password manager that oversells its
guarantees is more dangerous than one that's honest about its limits.

## Assets

1. The master password (never stored, anywhere, in any form).
2. The derived vault key (in-memory only while unlocked; zeroized on
   drop/lock).
3. The vault file's plaintext contents once decrypted (credentials,
   notes).
4. The encrypted vault file at rest.
5. Data in transit between the desktop app and (in the target
   architecture) a browser extension.

## Actors

- **A malicious or compromised website.** Can run arbitrary JavaScript in
  its own origin, control its own DOM/forms, and — the interesting case —
  register a domain designed to *look like* a legitimate one.
- **A malicious browser extension** (someone else's, not KeyFlow's).
- **Local malware / another OS-level process** running as the same or a
  different OS user.
- **Someone with physical or remote-shell access to the unlocked
  machine**, including anyone else logged into the same OS user account.
- **A compromised or malicious future sync server** (KeyFlow has no
  sync server today — see PRIVACY.md — but the architecture is
  documented for when one might exist).
- **Whoever supplies an imported file** (CSV, JSON) — treated as
  adversarial input, not trusted data.

## What KeyFlow protects against today

| Threat | Mitigation |
|---|---|
| Vault file stolen (disk theft, backup leak, synced-folder leak) | AES-256-GCM encryption under an Argon2id-derived key; without the master password, the file is ciphertext plus a non-secret salt and cost parameters. See SECURITY.md for parameters. |
| Vault file tampered with at rest | AES-GCM's authentication tag detects any modification; `Vault::unlock` returns `AuthenticationFailed`/`CorruptVault` rather than silently decrypting garbage. Verified by `crates/keyflow-core/src/vault.rs` test `tampered_vault_file_fails_to_unlock`. |
| Phishing via a look-alike domain (`example.com.attacker.com`, `attacker-example.com`, homograph-adjacent Unicode/punycode confusion) | `keyflow_core::domain::evaluate_match` does label-boundary eTLD+1 matching, never a naive substring/suffix check, and normalizes IDNA/punycode before comparing. 16 unit tests specifically target adversarial domain shapes. |
| A credential silently offered on the wrong origin | Matching is a hard allow-list operation (`ExactMatch` / `SubdomainMatch` only); everything else is `NoMatch` or an explicit `Blocked{reason}` the UI is expected to surface, never a silent no-op that could be confused with "nothing saved here." |
| HTTPS→HTTP downgrade before offering a saved login | Explicitly detected and returned as `Blocked`, not merely `NoMatch` — see `https_to_http_downgrade_is_blocked_not_matched`. |
| A credential saved against a bare public suffix acting as a wildcard | `is_public_suffix` guard rejects matching when the saved host is itself just a suffix (`co.uk`, `com`, etc.) — see `known_two_label_public_suffix_is_respected`. |
| Wrong master password | Every unlock attempt re-derives the key and must pass an AEAD authentication check; there is no partial-credit or timing-distinguishable failure mode between "wrong password" and "corrupted file" (both return the same error, deliberately — see below). |
| Secrets lingering in memory longer than necessary | `VaultKey` is `ZeroizeOnDrop`; `Credential::drop` zeroizes its password/notes fields; `crypto::open` zeroizes its intermediate plaintext buffer after copying out the result. |
| Secrets lingering on the clipboard | `copy_with_clipboard_timeout` clears the clipboard after a user-configurable interval, but only if the clipboard still holds exactly what KeyFlow put there (so it doesn't stomp on something the user copied afterward). |
| Accidental credential leakage into logs | `KeyflowError` variants never carry password/key material; nothing in `keyflow-core` calls a logging macro with credential contents. (There is no logging framework wired in yet at all — see ROADMAP — so this is "nothing to leak" rather than "leakage tested and blocked"; tightening this is tracked work.) |
| A crash mid-write corrupting the vault | Atomic write: content is written to a temp file, **read back and verified to decrypt**, then `rename()`'d over the target. A crash before `rename()` leaves the original file untouched. |
| Malicious/malformed CSV import | Parsed with the `csv` crate (not hand-rolled), every row validated independently (bad rows are skipped with a reported warning, not fatal), imported credentials go through the exact same domain-parsing and strength-estimation code as any other credential — no separate, less-trusted code path. |
| A web page lying about its own origin to get a credential offered | The browser extension's background script determines the origin from `sender.url`/`sender.origin` — fields the browser fills in based on the actual frame, not from anything the content script (which runs in page-influenced context) reports about itself. See ARCHITECTURE.md §4. |
| The browser extension being tricked into releasing a credential for the wrong site | `GetCredential` is re-validated against the current origin *inside the desktop app*, server-side, on every call — the extension's own earlier `FindMatches` result is never trusted as sufficient authorization on its own. |

## What KeyFlow does **not** protect against (yet, or ever)

Being explicit here matters more than being reassuring.

- **A compromised OS or root-level malware.** If an attacker can read
  arbitrary process memory or install a kernel-level keylogger, no
  desktop password manager — KeyFlow included — can protect the master
  password or decrypted secrets. This is true of every password manager
  and is not a KeyFlow-specific gap.
- **Memory scraping of a *running, unlocked* KeyFlow process.** Zeroizing
  on drop reduces the *window* during which secrets sit in memory, but
  while the vault is unlocked, decrypted credentials necessarily exist in
  process memory to be displayed/copied. A sufficiently privileged local
  attacker could scrape it. Locking the vault (manually, on auto-lock
  timeout, or on quit) closes this window; this release does not yet
  implement lock-on-system-sleep (see ROADMAP.md).
- **Forgotten master password.** By design, there is no recovery path
  that doesn't route through the master password — see SECURITY.md
  "Why there is no password reset." This is a deliberate trade-off
  disclosed during onboarding, not an oversight.
- **A malicious browser extension already installed by the user.**
  Native messaging is scoped to KeyFlow's own extension by a pinned
  extension ID in the native-messaging host manifest's `allowed_origins`
  (see `browser-extension/chrome/manifest.json`'s `key` field), but a
  different, unrelated malicious extension with broad page-content
  permissions could still scrape a form *after* KeyFlow has filled it —
  the same is true of every autofill mechanism, browser built-in ones
  included.
- **Any local process (not just the browser) directly connecting to the
  local agent socket.** This is new with the browser extension
  (ARCHITECTURE.md §4) and is a genuine, if modest, increase in local
  attack surface: the Unix domain socket / named pipe the native
  messaging host talks to authenticates nothing beyond "runs as the same
  OS user" — the same trust boundary the OS-keychain quick-unlock
  feature already relies on, but reachable with three lines of socket
  code instead of needing to know a Keychain service/account name. A
  local program (malware, or another process you're running) that
  already runs as you could speak this protocol directly, skip the
  browser and native host entirely, and ask for a credential by guessing
  an id and origin — no signature or token protects it. Two things limit
  the damage: it only works while the vault is actually unlocked (the
  same condition under which secrets already sit in process memory
  anyway — see "memory scraping" above), and `GetCredential` re-validates
  the origin server-side rather than trusting the caller. This is a
  fundamental limitation of native-messaging-based browser integration
  generally — every password manager that does this has the same
  boundary — not something unique to KeyFlow's implementation. See
  `apps/desktop/src-tauri/src/agent_server.rs`'s module doc for the full
  reasoning.
- **A malicious or compromised KeyFlow update** (supply-chain compromise
  of the build pipeline). Mitigation is process (signed releases from
  reviewed CI, see ROADMAP.md for the auto-update signing work) rather
  than something the app can defend against at runtime.
- **Full homograph/confusable-script phishing domains.** `Origin::parse`
  normalizes to punycode before comparing, which stops *inconsistent*
  Unicode representations of the *same* domain from bypassing matching,
  but it does not flag `аpple.com` (Cyrillic а) as suspicious the way a
  browser's own address bar heuristics might. This is a documented gap,
  not a false sense of security — tracked in ROADMAP.md.
- **A quick-unlock secret stored in the OS keychain being extracted by
  anything else logged into the same OS account.** This is an explicit,
  disclosed, opt-in, off-by-default trade-off — see SECURITY.md.
- **Compromise of a future sync server.** No sync server exists in this
  release. If/when one is built, the architecture in ARCHITECTURE.md §
  "Optional sync" requires it to be zero-knowledge (never receiving
  plaintext, the master password, or the derived key) — but until that
  code exists and is audited, this line item is aspirational, not a
  current guarantee.

## Why "wrong password" and "corrupted file" return the same error

`KeyflowError::AuthenticationFailed` is returned for both a wrong master
password and a genuinely corrupted/tampered vault file. This is
intentional: distinguishing them would require decrypting *further* on a
partial signal, which is exactly the kind of oracle that has broken
authenticated-encryption schemes before (padding-oracle-style bugs, just
generalized). The UI-facing message ("incorrect master password or
corrupted vault") is intentionally the same for both cases too.

## Fuzzing and adversarial-input targets

Tracked in ROADMAP.md as not-yet-implemented for this release (unit
tests currently cover the specific adversarial cases enumerated above by
hand, which is narrower than property-based fuzzing): the domain parser,
the CSV import parser, and vault-file JSON deserialization are the three
highest-value fuzz targets, since all three consume attacker-influenced
input (a visited URL, an imported file, and — in principle — a vault file
that could have been tampered with on disk before an unlock attempt).

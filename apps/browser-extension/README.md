# Browser extension

## Status

**Chrome / Edge / Chromium (Manifest V3)**: implemented, in
`chrome/`. Every piece — form detection, the native messaging host, the
local agent socket in the desktop app — has been tested individually
(see ROADMAP.md for exactly what was and wasn't verified end-to-end).
It is not published anywhere; you load it as an unpacked extension.

**Firefox and Safari**: not implemented. Firefox uses a different native
messaging manifest format and location (`allowed_extensions` instead of
`allowed_origins`, keyed by extension ID from a different manifest
field); Safari doesn't use Chrome-style native messaging at all — a
Safari Web Extension talks to its containing macOS app directly. Both
are architecture-compatible with the design below, just not wired up.

## How it works

See [ARCHITECTURE.md](../ARCHITECTURE.md) §4 for the full request-path
diagram. In short: the content script detects login-shaped forms and
shows a small suggestion UI; the background service worker determines
the page's *real* origin (from the browser's own `sender.url`, never
from anything the page itself claims) and relays exactly two questions —
"what matches this origin?" and "give me this one credential" — through
`keyflow-native-host` (a small Rust binary Chrome spawns per request) to
a local socket inside the running KeyFlow desktop app. The extension
never receives the master password or vault key, only the one credential
the user explicitly picked, and only after the desktop app re-checks
that it still matches the current origin.

## Setting it up locally

1. Build the extension:
   ```bash
   cd apps/browser-extension/chrome
   npm install
   npm run build
   ```
2. Build the native host binary (part of the main workspace):
   ```bash
   cargo build --release -p keyflow-native-host
   ```
3. In KeyFlow, go to **Settings → Browser extension → Enable for
   installed browsers**. This writes the native-messaging manifest
   (pointing at the binary you just built) into whichever supported
   browsers' `NativeMessagingHosts` directories exist on your machine —
   see `apps/desktop/src-tauri/src/browser_extension.rs`.
4. In Chrome (or Edge/Brave/Chromium), open `chrome://extensions`,
   enable **Developer mode**, click **Load unpacked**, and select
   `apps/browser-extension/chrome`.
5. Restart the browser so it picks up the newly registered native
   messaging host, then visit any page with a login form.

**If "Load unpacked" is greyed out or Developer mode is disabled by
policy** ("This setting is managed by your administrator"), your
organization has disabled unpacked extension loading — that's an
organizational Chrome policy, not something KeyFlow can work around, and
it's exactly what happened on the machine this extension was developed
on, which is why the fully-assembled extension-in-real-Chrome flow
couldn't be verified end-to-end here (see ROADMAP.md).

## The pinned extension ID

`manifest.json`'s `key` field is a real RSA public key. Chrome computes
an extension's ID deterministically from the SHA-256 hash of this key,
*even for an unpacked load* — without it, reloading the unpacked
extension after every rebuild would assign a new random ID, and the
native-messaging manifest's `allowed_origins` would go stale each time.
The ID this key produces is `lkljkibkbdbgjmljnhoioceloeiiigmj`, hardcoded
into both the manifest and `browser_extension.rs`'s registration logic.
The matching private key was used only to compute this ID and was not
retained — it has no other purpose (it is not a code-signing key for
anything else, and does not grant Chrome Web Store publishing rights,
which uses Google's own separate signing flow).

## Non-negotiables for anyone extending this

1. **Never send the master password or derived vault key to the
   extension**, at any point, under any code path.
2. **Determine origin from the browser's own APIs** (`sender.url` /
   `sender.origin`), never from `document.location` or anything content-
   script-reported that page JavaScript could influence.
3. **No autofill without a fresh round trip for the exact current
   origin, every time** — never cache a "yes" decision across
   navigations.
4. **Re-validate the origin server-side** before releasing a credential
   (see `agent_server.rs::handle_get_credential`) — never trust that an
   earlier `FindMatches` call is still accurate.
5. **Reuse `keyflow_core::domain` as-is** from the desktop app process;
   never reimplement matching logic in the extension's own TypeScript.

## Known limitations

- Closed shadow-DOM form fields are invisible to the content script —
  a universal extension-API limitation, not fixable here.
- The local agent socket authenticates nothing beyond "runs as the same
  OS user" — see THREAT_MODEL.md's "local agent socket" entry.
- No auto-launch of the desktop app if it isn't already running; the
  extension just reports it can't connect.
- Detection heuristics (`content.ts`) are real and tested against several
  form shapes (see ROADMAP.md), but won't catch every login form on the
  web — false negatives (no suggestion shown) are far more likely than
  false positives, which was the deliberate design bias.

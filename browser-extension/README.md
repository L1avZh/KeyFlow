# Browser extension — architecture only, not implemented

This directory intentionally contains no extension code yet. It exists
to hold the design so it isn't lost, and so contributors picking up
ROADMAP.md item #1 have a concrete starting point rather than a blank
page.

Why nothing runnable is checked in here: a browser extension is the part
of KeyFlow with the highest blast radius if built carelessly (it runs
inside every web page you visit), and this milestone's time was spent
making sure `keyflow-core`'s matching engine and the desktop app around
it are genuinely solid — including 16 unit tests specifically covering
phishing-domain edge cases the extension will eventually rely on. Ship a
matching engine that's been trying to lie to itself with fake passing
tests, or a browser extension whose content-script form detection is
mocked/stubbed, and it's a real hazard to whoever installs it thinking it
works. Better to say plainly: not built yet.

## Target architecture

See [ARCHITECTURE.md](../ARCHITECTURE.md) §4 for the full data-flow
diagram. Summary:

```
Content script (per-tab)          Detects login form fields using multiple
                                   signals — autocomplete attributes, input
                                   type, <label> text, name/id patterns,
                                   surrounding text, iframe origin — never
                                   relying on any single signal.
        │
        ▼
Background / service worker       The only place that knows the page's real
                                   origin (from the browser's own APIs, not
                                   from page JS the page itself could lie
                                   about — this is what makes phishing
                                   resistance possible at all).
        │  native messaging (chrome.runtime.connectNative /
        │  Firefox's equivalent) — framed request/response, no polling
        ▼
KeyFlow desktop app                Runs Origin::parse(page_origin) and
                                   evaluate_match(...) from keyflow-core —
                                   the SAME matching engine the desktop UI's
                                   "Autofill Tester" already exercises.
                                   Never receives page contents; only the
                                   origin string.
        │  response: matched credential(s), or a block reason
        ▼
Content script                    Fills the form only after this round-trip
                                   completes, and (per the project brief)
                                   never silently — the user picks an
                                   account from a suggestion UI first.
```

## Non-negotiables for whoever builds this

1. **The extension never receives the master password or the derived
   vault key**, at any point, under any code path. It receives, at most,
   the specific credential(s) the desktop app has already decided are
   safe for the current origin.
2. **Origin comes from the browser's extension APIs**, never from
   `document.location` or anything the page's own JavaScript could
   spoof.
3. **No autofill without a round-trip to the desktop app for that exact
   origin, every time** — no caching a "yes" decision across
   navigations, since the origin can change between page loads on
   what looks like "the same tab" to a user (e.g. a redirect chain).
4. **Native messaging host manifests** must be scoped to KeyFlow's own
   extension ID(s) specifically, per browser vendor requirements — not a
   wildcard.
5. Reuse `keyflow-core::domain` as-is from the desktop app process; do
   not reimplement matching logic in the extension's own
   TypeScript/JavaScript. One engine, one set of tests.

## Planned browser support

Manifest V3 for Chrome/Edge/Chromium-based browsers and Firefox (Firefox
now supports MV3), from one shared TypeScript codebase where the
platform differences are isolated to the native-messaging transport and
manifest metadata. Safari support is a stretch goal — Safari Web
Extensions have historically lagged MV3 feature parity and have their
own native-messaging equivalent (via the containing macOS app), which
would need its own thin transport shim.

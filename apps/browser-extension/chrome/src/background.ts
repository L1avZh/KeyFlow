// Background service worker: the only place in the extension that knows
// which OS process to talk to (the native messaging host) and the only
// place that determines the page's real origin. Content scripts never
// see the native host directly and never get to assert their own origin
// — see types.ts for why.

import { AgentResponse, ContentMessage, NATIVE_HOST_NAME } from "./types.js";

function nativeRequest(payload: Record<string, unknown>): Promise<AgentResponse> {
  return new Promise((resolve) => {
    chrome.runtime.sendNativeMessage(NATIVE_HOST_NAME, payload, (response) => {
      if (chrome.runtime.lastError) {
        resolve({
          type: "error",
          message: friendlyNativeError(chrome.runtime.lastError.message ?? "unknown native messaging error"),
        });
        return;
      }
      resolve(response as AgentResponse);
    });
  });
}

function friendlyNativeError(raw: string): string {
  if (raw.includes("not found") || raw.includes("not installed")) {
    return "KeyFlow's browser connector isn't installed. Open KeyFlow → Settings → Browser Extension to enable it.";
  }
  if (raw.includes("Native host has exited") || raw.includes("Error when communicating")) {
    return "Lost the connection to KeyFlow. Is the KeyFlow desktop app still open?";
  }
  return raw;
}

function originFromSender(sender: chrome.runtime.MessageSender): string | null {
  // `sender.origin` is the more direct field when present, but isn't
  // populated for every sender type across all Chrome versions; `url`
  // is always present for a content-script sender and is at least as
  // authoritative (it's the frame's actual navigated URL, filled in by
  // the browser itself — not something the page's own JS can override
  // to lie to us).
  const source = sender.origin ?? sender.url;
  if (!source) return null;
  try {
    return new URL(source).origin;
  } catch {
    return null;
  }
}

chrome.runtime.onMessage.addListener((message: ContentMessage, sender, sendResponse) => {
  const origin = originFromSender(sender);
  if (!origin) {
    sendResponse({ type: "error", message: "couldn't determine this page's origin" } satisfies AgentResponse);
    return false;
  }

  (async () => {
    switch (message.cmd) {
      case "ping": {
        const response = await nativeRequest({ type: "ping" });
        sendResponse(response);
        break;
      }
      case "findMatches": {
        const response = await nativeRequest({ type: "find_matches", origin });
        sendResponse(response);
        break;
      }
      case "getCredential": {
        const response = await nativeRequest({ type: "get_credential", id: message.id, origin });
        sendResponse(response);
        break;
      }
    }
  })();

  return true; // keep the message channel open for the async sendResponse above
});

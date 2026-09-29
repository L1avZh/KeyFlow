// Mirrors keyflow-agent's AgentRequest/AgentResponse (Rust) shapes for
// the messages that travel content-script -> background -> native host
// -> desktop app and back. Keeping the wire shape identical end-to-end
// means background.ts can forward native-host responses to the content
// script mostly unchanged, rather than translating between two parallel
// but subtly different message vocabularies.

export interface MatchSummary {
  id: string;
  name: string;
  username: string;
  decision: string;
}

export type AgentResponse =
  | { type: "pong"; unlocked: boolean }
  | { type: "matches"; items: MatchSummary[] }
  | { type: "credential"; username: string; password: string }
  | { type: "error"; message: string };

// Content script -> background. Deliberately does NOT carry an origin —
// the background script determines that itself from `sender.url`, which
// the browser fills in based on the actual frame the message came from
// and which page JavaScript cannot spoof. Trusting a page-supplied
// origin here would defeat the entire point of doing this check in the
// extension rather than the page.
export type ContentMessage =
  | { cmd: "ping" }
  | { cmd: "findMatches" }
  | { cmd: "getCredential"; id: string };

export const NATIVE_HOST_NAME = "app.keyflow.native_host";

import { NATIVE_HOST_NAME } from "./types.js";
import type { AgentResponse } from "./types.js";

const statusEl = document.getElementById("status")!;

chrome.runtime.sendNativeMessage(NATIVE_HOST_NAME, { type: "ping" }, (response: AgentResponse) => {
  if (chrome.runtime.lastError) {
    statusEl.textContent = "KeyFlow desktop app isn't reachable. Open KeyFlow and enable the browser extension in Settings.";
    statusEl.classList.add("bad");
    return;
  }
  if (response?.type === "pong") {
    statusEl.textContent = response.unlocked ? "Connected — vault unlocked." : "Connected — vault is locked.";
    statusEl.classList.add("ok");
  } else {
    statusEl.textContent = "Connected, but got an unexpected response.";
  }
});

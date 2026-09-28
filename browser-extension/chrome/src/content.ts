// Content script: detects login-shaped forms on the page and offers a
// small, dismissible suggestion UI near them. This file never talks to
// the native host directly — every request goes through the background
// script via chrome.runtime.sendMessage, so the background script (which
// the browser gives an authoritative origin for) is what actually gates
// what gets released. See ARCHITECTURE.md §4 and
// browser-extension/README.md for the non-negotiables this follows:
// no autofill without a fresh per-origin round trip, no caching a
// decision across navigations, and the extension never handles the
// master password or vault key — only, after the user explicitly picks
// an account, the one credential's username/password.

// Deliberately no import from ./types.ts, even a type-only one: Chrome
// loads content scripts as classic (non-module) scripts, and a type-only
// import with nothing left at runtime still makes tsc emit a trailing
// `export {}` to mark the output as an ES module — which is a syntax
// error in the non-module context content scripts actually execute in.
// (Caught by actually injecting the compiled output into a real page
// during testing rather than only type-checking it.)
interface MatchSummary {
  id: string;
  name: string;
  username: string;
  decision: string;
}
type AgentResponse =
  | { type: "pong"; unlocked: boolean }
  | { type: "matches"; items: MatchSummary[] }
  | { type: "credential"; username: string; password: string }
  | { type: "error"; message: string };

const PROCESSED_ATTR = "data-keyflow-processed";
const HOST_ELEMENT_ID = "keyflow-suggestion-host";

// ---------- Field classification ----------
// Multiple independent signals, each contributing points, rather than
// trusting any single attribute — real forms are inconsistent about
// which of these they bother to set correctly.

function textSignal(el: HTMLInputElement): string {
  const label = labelTextFor(el);
  return [
    el.autocomplete,
    el.name,
    el.id,
    el.placeholder,
    el.getAttribute("aria-label") ?? "",
    label,
  ]
    .join(" ")
    .toLowerCase();
}

function labelTextFor(el: HTMLInputElement): string {
  if (el.labels && el.labels.length > 0) {
    return Array.from(el.labels)
      .map((l) => l.textContent ?? "")
      .join(" ");
  }
  // Fall back to a nearby preceding label-like element when there's no
  // programmatic <label for>. Cheap heuristic: look at the previous
  // sibling and the parent's previous sibling.
  const candidates: (Element | null)[] = [el.previousElementSibling, el.parentElement?.previousElementSibling ?? null];
  return candidates
    .filter((c): c is Element => !!c)
    .map((c) => c.textContent ?? "")
    .join(" ");
}

function isPasswordField(el: HTMLInputElement): boolean {
  if (el.type === "password") return true;
  return false;
}

function isUsernameLikeField(el: HTMLInputElement): boolean {
  if (el.type !== "text" && el.type !== "email" && el.type !== "tel") return false;
  const text = textSignal(el);
  const positive = ["user", "email", "e-mail", "login", "account", "identifier", "signin", "sign-in"];
  const negative = ["search", "coupon", "promo", "zip", "postal", "phone-extra", "otp", "one-time", "verification"];
  if (negative.some((n) => text.includes(n))) return false;
  if (el.autocomplete === "username" || el.autocomplete === "email") return true;
  return positive.some((p) => text.includes(p));
}

function isNewPasswordField(el: HTMLInputElement): boolean {
  const text = textSignal(el);
  if (el.autocomplete === "new-password") return true;
  return ["new password", "new-password", "create password", "choose password", "confirm password", "confirm-password"].some(
    (p) => text.includes(p)
  );
}

interface DetectedForm {
  container: HTMLElement;
  passwordField: HTMLInputElement;
  usernameField: HTMLInputElement | null;
}

function findFormGroupRoot(field: HTMLInputElement): HTMLElement {
  const formAncestor = field.closest("form");
  if (formAncestor) return formAncestor;
  // No <form> wrapper (common in SPAs). Walk a bounded number of
  // ancestors looking for the smallest container that holds another
  // candidate input too — falling all the way back to <body> would lump
  // this field's "form" together with every other unrelated form on the
  // page, and hand out a completely wrong field as the username. Caught
  // via actually testing detection against a page with two independent
  // login-shaped groups, not just against a single isolated one.
  let node: HTMLElement | null = field.parentElement;
  for (let depth = 0; node && depth < 6; depth++, node = node.parentElement) {
    if (node.querySelectorAll("input").length >= 2) return node;
  }
  return field.parentElement ?? document.body;
}

function collectCandidateInputs(root: ParentNode): HTMLInputElement[] {
  const results: HTMLInputElement[] = [];
  const walk = (node: ParentNode) => {
    for (const el of node.querySelectorAll("input")) {
      results.push(el as HTMLInputElement);
      const shadow = (el as HTMLElement).shadowRoot;
      if (shadow) walk(shadow);
    }
    // Pierce open shadow roots on non-input elements too (custom
    // form-field wrapper components are common). Closed shadow roots
    // are, by design, inaccessible to any extension — a known,
    // universal limitation, not something fixable here.
    for (const el of node.querySelectorAll("*")) {
      const shadow = (el as HTMLElement).shadowRoot;
      if (shadow) walk(shadow);
    }
  };
  walk(root);
  return results;
}

function detectForms(): DetectedForm[] {
  const inputs = collectCandidateInputs(document).filter((el) => !el.disabled && el.type !== "hidden");
  const passwordFields = inputs.filter(isPasswordField).filter((el) => !isNewPasswordField(el));

  const forms: DetectedForm[] = [];
  for (const pw of passwordFields) {
    if (pw.hasAttribute(PROCESSED_ATTR)) continue;
    const root = findFormGroupRoot(pw);
    const siblingInputs = collectCandidateInputs(root);
    const usernameField =
      siblingInputs.find((el) => el !== pw && isUsernameLikeField(el)) ??
      // Fall back to the nearest preceding text/email input even without
      // a strong textual signal — many login forms genuinely have no
      // helpful attributes at all beyond field order.
      siblingInputs.find((el) => el !== pw && (el.type === "text" || el.type === "email") && el.compareDocumentPosition(pw) & Node.DOCUMENT_POSITION_FOLLOWING) ??
      null;
    forms.push({ container: root, passwordField: pw, usernameField });
    pw.setAttribute(PROCESSED_ATTR, "1");
  }
  return forms;
}

// ---------- Filling ----------

function setNativeValue(el: HTMLInputElement, value: string) {
  const prototype = Object.getPrototypeOf(el);
  const descriptor = Object.getOwnPropertyDescriptor(prototype, "value");
  descriptor?.set?.call(el, value);
  el.dispatchEvent(new Event("input", { bubbles: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
}

function fillCredential(form: DetectedForm, username: string, password: string) {
  if (form.usernameField && username) setNativeValue(form.usernameField, username);
  setNativeValue(form.passwordField, password);
}

// ---------- Suggestion UI (Shadow DOM, isolated from page CSS) ----------

let panelHost: HTMLElement | null = null;

function closePanel() {
  panelHost?.remove();
  panelHost = null;
}

function sendToBackground<T = AgentResponse>(message: unknown): Promise<T> {
  return new Promise((resolve) => {
    chrome.runtime.sendMessage(message, (response) => resolve(response as T));
  });
}

function positionNear(el: HTMLElement, panel: HTMLElement) {
  const rect = el.getBoundingClientRect();
  panel.style.position = "fixed";
  panel.style.top = `${rect.bottom + 6}px`;
  panel.style.left = `${Math.max(8, rect.right - 300)}px`;
}

async function openPanel(anchor: HTMLInputElement, form: DetectedForm) {
  closePanel();

  const host = document.createElement("div");
  host.id = HOST_ELEMENT_ID;
  host.style.all = "initial";
  host.style.position = "fixed";
  host.style.zIndex = "2147483647";
  document.documentElement.appendChild(host);
  panelHost = host;

  const shadow = host.attachShadow({ mode: "open" });
  const style = document.createElement("style");
  style.textContent = `
    .panel { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; width: 300px;
      background: #1c1e27; color: #eef0f7; border-radius: 12px; box-shadow: 0 12px 32px rgba(0,0,0,0.35);
      border: 1px solid #2b2d3a; overflow: hidden; font-size: 13px; }
    .header { display:flex; align-items:center; gap:8px; padding: 10px 12px; font-weight:600; border-bottom: 1px solid #2b2d3a; }
    .item { display:flex; flex-direction:column; gap:2px; padding: 9px 12px; cursor:pointer; }
    .item:hover { background: #262836; }
    .name { font-weight:600; }
    .sub { color:#a7abc0; font-size:11.5px; }
    .empty, .error { padding: 14px 12px; color:#a7abc0; line-height:1.4; }
    .error { color:#f28b82; }
  `;
  shadow.appendChild(style);

  const panel = document.createElement("div");
  panel.className = "panel";
  panel.innerHTML = `<div class="header">🔑 KeyFlow</div><div class="body">Loading…</div>`;
  shadow.appendChild(panel);
  positionNear(anchor, panel);

  const body = panel.querySelector(".body") as HTMLElement;
  const response = await sendToBackground({ cmd: "findMatches" });
  renderBody(body, response, form);

  // Dismiss on outside click or Escape.
  const onDocClick = (e: MouseEvent) => {
    if (!host.contains(e.target as Node)) {
      closePanel();
      document.removeEventListener("mousedown", onDocClick, true);
    }
  };
  document.addEventListener("mousedown", onDocClick, true);
  document.addEventListener(
    "keydown",
    function onKey(e) {
      if (e.key === "Escape") {
        closePanel();
        document.removeEventListener("keydown", onKey, true);
      }
    },
    true
  );
}

function renderBody(body: HTMLElement, response: AgentResponse, form: DetectedForm) {
  if (!response || response.type === "error") {
    body.innerHTML = `<div class="error">${escapeHtml(response?.message ?? "Something went wrong.")}</div>`;
    return;
  }
  if (response.type !== "matches") {
    body.innerHTML = `<div class="error">Unexpected response.</div>`;
    return;
  }
  const items: MatchSummary[] = response.items;
  if (items.length === 0) {
    body.innerHTML = `<div class="empty">No saved KeyFlow logins for this site.</div>`;
    return;
  }
  body.innerHTML = "";
  for (const item of items) {
    const row = document.createElement("div");
    row.className = "item";
    row.innerHTML = `<span class="name">${escapeHtml(item.name)}</span><span class="sub">${escapeHtml(item.username)}</span>`;
    row.addEventListener("click", async () => {
      body.innerHTML = `<div class="empty">Filling…</div>`;
      const credResponse = await sendToBackground({ cmd: "getCredential", id: item.id });
      if (credResponse?.type === "credential") {
        fillCredential(form, credResponse.username, credResponse.password);
        closePanel();
      } else {
        renderBody(body, credResponse, form);
      }
    });
    body.appendChild(row);
  }
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));
}

// ---------- Wiring: a small indicator on focus, full panel on click ----------

function attachTrigger(form: DetectedForm) {
  const anchor = form.usernameField ?? form.passwordField;
  anchor.addEventListener("focus", () => {
    showIndicator(anchor, form);
  });
}

let indicatorEl: HTMLElement | null = null;

function showIndicator(anchor: HTMLInputElement, form: DetectedForm) {
  indicatorEl?.remove();
  const host = document.createElement("div");
  host.style.all = "initial";
  host.style.position = "fixed";
  host.style.zIndex = "2147483647";
  const rect = anchor.getBoundingClientRect();
  host.style.top = `${rect.top + rect.height / 2 - 10}px`;
  host.style.left = `${rect.right - 26}px`;
  document.documentElement.appendChild(host);
  indicatorEl = host;

  const shadow = host.attachShadow({ mode: "open" });
  const btn = document.createElement("button");
  btn.textContent = "🔑";
  btn.title = "Fill with KeyFlow";
  btn.style.cssText =
    "width:20px;height:20px;border-radius:6px;border:none;background:#2b2d3a;cursor:pointer;font-size:12px;line-height:1;padding:0;";
  btn.addEventListener("mousedown", (e) => e.preventDefault()); // don't steal focus from the field
  btn.addEventListener("click", () => openPanel(anchor, form));
  shadow.appendChild(btn);

  const remove = () => {
    if (document.activeElement !== anchor) {
      host.remove();
      if (indicatorEl === host) indicatorEl = null;
    }
  };
  anchor.addEventListener("blur", () => setTimeout(remove, 150), { once: true });
}

function scan() {
  const forms = detectForms();
  for (const form of forms) attachTrigger(form);
}

scan();
// Re-scan as the page mutates — most modern login forms render some
// time after initial document load (SPA routing, lazy-loaded widgets).
const observer = new MutationObserver(() => scan());
observer.observe(document.documentElement, { childList: true, subtree: true });

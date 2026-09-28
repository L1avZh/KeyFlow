import { api } from "../api";
import { setRoute, state } from "../state";
import { toast } from "../toast";
import type { Credential } from "../types";

type Action = {
  label: string;
  hint: string;
  run: () => void | Promise<void>;
};

let openPalette: (() => void) | null = null;
// Tracks the currently-open palette's own close function, if any. Used
// both to make a second shortcut-press toggle the palette closed instead
// of stacking a duplicate on top, and to guarantee the per-open Escape
// listener below always gets torn down — previously it was only removed
// when the user dismissed via Escape specifically; dismissing by
// clicking away or picking an item leaked it on `document` forever
// (an unbounded, ever-growing listener across a long session).
let closeCurrent: (() => void) | null = null;

export function initCommandPalette(navigate: () => void) {
  function open() {
    if (closeCurrent) {
      closeCurrent();
      return;
    }

    const backdrop = document.createElement("div");
    backdrop.className = "kf-palette-backdrop";
    backdrop.innerHTML = `
      <div class="kf-palette" role="dialog" aria-label="Quick access">
        <input class="kf-palette-input" placeholder="Search KeyFlow… (credentials, or an action)" autofocus />
        <div class="kf-palette-results"></div>
      </div>
    `;
    document.body.appendChild(backdrop);
    const input = backdrop.querySelector<HTMLInputElement>(".kf-palette-input")!;
    const results = backdrop.querySelector<HTMLElement>(".kf-palette-results")!;

    const onEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    const close = () => {
      document.removeEventListener("keydown", onEscape);
      backdrop.remove();
      if (closeCurrent === close) closeCurrent = null;
    };
    closeCurrent = close;

    backdrop.addEventListener("click", (e) => {
      if (e.target === backdrop) close();
    });
    document.addEventListener("keydown", onEscape);

    const staticActions: Action[] = [
      { label: "Open Vault", hint: "Navigate", run: () => { setRoute("vault"); navigate(); } },
      { label: "Generate password", hint: "Navigate", run: () => { setRoute("generator"); navigate(); } },
      { label: "Open Security dashboard", hint: "Navigate", run: () => { setRoute("security"); navigate(); } },
      { label: "Open Settings", hint: "Navigate", run: () => { setRoute("settings"); navigate(); } },
      { label: "Lock KeyFlow", hint: "Security", run: async () => { await api.lockVault(); location.reload(); } },
    ];

    function draw() {
      const q = input.value.toLowerCase().trim();
      const credMatches: Credential[] = q
        ? state.credentials.filter(
            (c) => c.name.toLowerCase().includes(q) || c.url.toLowerCase().includes(q) || c.username.toLowerCase().includes(q)
          )
        : state.credentials.slice(0, 5);
      const actionMatches = staticActions.filter((a) => !q || a.label.toLowerCase().includes(q));

      const rows: string[] = [];
      for (const c of credMatches) {
        rows.push(
          `<div class="kf-palette-item" data-cred="${c.id}"><span>🔑 ${escapeHtml(c.name)}</span><span class="kf-cred-sub">${escapeHtml(c.username)}</span></div>`
        );
      }
      for (const [i, a] of actionMatches.entries()) {
        rows.push(`<div class="kf-palette-item" data-action="${i}"><span>⚡ ${a.label}</span><span class="kf-cred-sub">${a.hint}</span></div>`);
      }
      results.innerHTML = rows.join("") || `<div class="kf-palette-item">No matches</div>`;

      results.querySelectorAll<HTMLElement>("[data-cred]").forEach((el) => {
        el.addEventListener("click", async () => {
          const cred = state.credentials.find((c) => c.id === el.dataset.cred)!;
          await api.copyWithClipboardTimeout(cred.password, state.clipboardClearSeconds);
          await api.touchCredentialUsed(cred.id);
          toast(`Copied password for ${cred.name}.`);
          close();
        });
      });
      results.querySelectorAll<HTMLElement>("[data-action]").forEach((el) => {
        el.addEventListener("click", () => {
          actionMatches[Number(el.dataset.action)].run();
          close();
        });
      });
    }

    input.addEventListener("input", draw);
    draw();
    input.focus();
  }

  openPalette = open;

  document.addEventListener("keydown", (e) => {
    const meta = e.metaKey || e.ctrlKey;
    if (meta && e.shiftKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      open();
    }
  });
  document.addEventListener("keyflow:open-palette", () => open());
}

export function triggerPalette() {
  openPalette?.();
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));
}

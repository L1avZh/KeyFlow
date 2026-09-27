import { api } from "../api";
import { openCredentialForm } from "../components/credentialForm";
import { state, subscribe } from "../state";
import type { SecurityOverview } from "../types";

export function renderHome(root: HTMLElement, navigate: (r: any) => void) {
  root.innerHTML = `
    <div class="kf-page-header">
      <div><h1>Home</h1><p>A quick look at your vault.</p></div>
    </div>
    <div class="kf-stat-grid" id="home-stats"></div>
    <div style="display:flex; gap:12px; margin-bottom:24px">
      <button class="kf-btn kf-btn-primary" id="home-add">+ Add login</button>
      <button class="kf-btn" id="home-generate">Generate password</button>
      <button class="kf-btn" id="home-palette">⌘⇧K Quick access</button>
    </div>
    <h2 style="font-size:14px; margin-bottom:10px">Recently used</h2>
    <div class="kf-cred-list" id="home-recent"></div>
  `;

  root.querySelector("#home-add")!.addEventListener("click", () => openCredentialForm());
  root.querySelector("#home-generate")!.addEventListener("click", () => navigate("generator"));
  root.querySelector("#home-palette")!.addEventListener("click", () =>
    document.dispatchEvent(new CustomEvent("keyflow:open-palette"))
  );

  // Captured once, up front — not re-queried inside draw(). draw() awaits
  // an IPC round-trip, and the user can navigate to another page before
  // it resolves; re-querying `root` at that point would find these ids
  // gone (replaced by whatever page is now mounted) and throw. Writing
  // to a stale-but-still-referenced node after navigating away is a
  // harmless no-op instead.
  const statsEl = root.querySelector<HTMLElement>("#home-stats")!;
  const recentEl = root.querySelector<HTMLElement>("#home-recent")!;

  async function draw() {
    const overview: SecurityOverview = await api.securityOverview();
    statsEl.innerHTML = `
      ${tile(overview.total, "Credentials", "ok")}
      ${tile(overview.weak, "Weak passwords", overview.weak > 0 ? "bad" : "ok")}
      ${tile(overview.reused, "Reused passwords", overview.reused > 0 ? "warn" : "ok")}
      ${tile(overview.old, "Old passwords (180d+)", overview.old > 0 ? "warn" : "ok")}
    `;
    statsEl.querySelectorAll<HTMLElement>("[data-nav]").forEach((el) =>
      el.addEventListener("click", () => navigate("security"))
    );

    const recent = [...state.credentials]
      .filter((c) => c.last_used_at)
      .sort((a, b) => new Date(b.last_used_at!).getTime() - new Date(a.last_used_at!).getTime())
      .slice(0, 5);
    if (recent.length === 0) {
      recentEl.innerHTML = `<p class="kf-muted">Nothing used yet — copy a password from the Vault to see it here.</p>`;
    } else {
      recentEl.innerHTML = recent
        .map(
          (c) => `
        <div class="kf-cred-row" style="cursor:default">
          <div class="kf-cred-avatar">${c.name[0]?.toUpperCase() ?? "?"}</div>
          <div class="kf-cred-main">
            <div class="kf-cred-name">${c.name}</div>
            <div class="kf-cred-sub">${c.username}</div>
          </div>
        </div>`
        )
        .join("");
    }
  }

  function tile(value: number, label: string, tone: "ok" | "warn" | "bad"): string {
    return `<div class="kf-stat-tile kf-stat-${tone}" data-nav="1"><div class="kf-stat-value">${value}</div><div class="kf-stat-label">${label}</div></div>`;
  }

  draw();
  return subscribe(draw);
}

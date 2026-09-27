import { api } from "../api";
import type { AutofillMatch, Credential, SecurityOverview } from "../types";

type Tab = "overview" | "weak" | "reused" | "old" | "duplicates" | "autofill-tester";

export function renderSecurity(root: HTMLElement) {
  let tab: Tab = "overview";

  root.innerHTML = `
    <div class="kf-page-header">
      <div><h1>Security</h1><p>KeyFlow never sends your passwords anywhere to check this — it's all computed locally.</p></div>
    </div>
    <div class="kf-tab-bar" id="sec-tabs">
      <button class="kf-tab active" data-tab="overview">Overview</button>
      <button class="kf-tab" data-tab="weak">Weak</button>
      <button class="kf-tab" data-tab="reused">Reused</button>
      <button class="kf-tab" data-tab="old">Old</button>
      <button class="kf-tab" data-tab="duplicates">Duplicates</button>
      <button class="kf-tab" data-tab="autofill-tester">Autofill Tester</button>
    </div>
    <div id="sec-content"></div>
  `;

  const tabsEl = root.querySelector<HTMLElement>("#sec-tabs")!;
  const contentEl = root.querySelector<HTMLElement>("#sec-content")!;

  tabsEl.querySelectorAll<HTMLButtonElement>(".kf-tab").forEach((btn) => {
    btn.addEventListener("click", () => {
      tabsEl.querySelectorAll(".kf-tab").forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      tab = btn.dataset.tab as Tab;
      draw();
    });
  });

  function credRow(c: Credential): string {
    return `<div class="kf-cred-row" style="cursor:default">
      <div class="kf-cred-avatar">${c.name[0]?.toUpperCase() ?? "?"}</div>
      <div class="kf-cred-main">
        <div class="kf-cred-name">${c.name}</div>
        <div class="kf-cred-sub">${c.username || c.url}</div>
      </div>
      <span class="kf-badge ${c.strength.band.includes("weak") ? "kf-badge-danger" : ""}">${c.strength.band}</span>
    </div>`;
  }

  async function draw() {
    if (tab === "overview") {
      const o: SecurityOverview = await api.securityOverview();
      contentEl.innerHTML = `
        <div class="kf-stat-grid">
          ${stat(o.total, "Total credentials", "ok")}
          ${stat(o.weak, "Weak passwords", o.weak ? "bad" : "ok")}
          ${stat(o.reused, "Reused passwords", o.reused ? "warn" : "ok")}
          ${stat(o.old, "Old passwords", o.old ? "warn" : "ok")}
          ${stat(o.duplicates, "Duplicate entries", o.duplicates ? "warn" : "ok")}
          ${stat(o.missing_password, "Missing passwords", o.missing_password ? "bad" : "ok")}
        </div>
        <p class="kf-muted">Weak = below ~45 bits of estimated entropy. Old = unchanged for 180+ days. All estimates are computed on-device from the passwords already in your unlocked vault.</p>
      `;
      return;
    }
    if (tab === "weak") {
      const creds = await api.weakCredentials();
      contentEl.innerHTML = list(creds, "No weak passwords found.");
      return;
    }
    if (tab === "old") {
      const creds = await api.oldCredentials();
      contentEl.innerHTML = list(creds, "No passwords older than 180 days.");
      return;
    }
    if (tab === "reused") {
      const groups = await api.reusedCredentialGroups();
      contentEl.innerHTML = groups.length
        ? groups.map((g) => `<h3 style="font-size:12px; color:var(--kf-text-muted); margin:14px 0 6px">Shared password (${g.length} logins)</h3>${list(g, "")}`).join("")
        : emptyMsg("No reused passwords found.");
      return;
    }
    if (tab === "duplicates") {
      const groups = await api.duplicateCredentialGroups();
      contentEl.innerHTML = groups.length
        ? groups.map((g) => `<h3 style="font-size:12px; color:var(--kf-text-muted); margin:14px 0 6px">Same site + username (${g.length} entries)</h3>${list(g, "")}`).join("")
        : emptyMsg("No duplicate entries found.");
      return;
    }
    renderAutofillTester(contentEl);
  }

  function stat(value: number, label: string, tone: "ok" | "warn" | "bad"): string {
    return `<div class="kf-stat-tile kf-stat-${tone}"><div class="kf-stat-value">${value}</div><div class="kf-stat-label">${label}</div></div>`;
  }
  function list(creds: Credential[], emptyText: string): string {
    if (creds.length === 0) return emptyMsg(emptyText);
    return `<div class="kf-cred-list">${creds.map(credRow).join("")}</div>`;
  }
  function emptyMsg(text: string): string {
    return text ? `<p class="kf-muted">${text}</p>` : "";
  }

  draw();
}

function renderAutofillTester(container: HTMLElement) {
  container.innerHTML = `
    <p class="kf-muted" style="margin-bottom:12px">
      This simulates the browser extension's origin check without a real browser: type a URL as if you'd
      navigated there, and see exactly which saved logins KeyFlow would (and wouldn't) offer, and why.
    </p>
    <div class="kf-row-gap" style="margin-bottom:16px">
      <input id="tester-url" class="kf-input kf-flex-1" placeholder="https://login.example.com" />
      <button class="kf-btn kf-btn-primary" id="tester-go">Check</button>
    </div>
    <div id="tester-results"></div>
  `;
  const input = container.querySelector<HTMLInputElement>("#tester-url")!;
  const resultsEl = container.querySelector<HTMLElement>("#tester-results")!;

  const run = async () => {
    if (!input.value) return;
    try {
      const matches: AutofillMatch[] = await api.findAutofillMatches(input.value);
      if (matches.length === 0) {
        resultsEl.innerHTML = `<p class="kf-muted">No saved login would be offered on this page.</p>`;
        return;
      }
      resultsEl.innerHTML = matches
        .map(
          (m) => `
        <div class="kf-cred-row" style="cursor:default">
          <div class="kf-cred-avatar">${m.credential.name[0]?.toUpperCase() ?? "?"}</div>
          <div class="kf-cred-main">
            <div class="kf-cred-name">${m.credential.name}</div>
            <div class="kf-cred-sub">${m.credential.username} · saved for ${m.credential.url}</div>
          </div>
          <span class="kf-badge kf-badge-success">${m.decision}</span>
        </div>`
        )
        .join("");
    } catch (e) {
      resultsEl.innerHTML = `<div class="kf-blocked-banner">⚠️ ${String(e)}</div>`;
    }
  };
  container.querySelector("#tester-go")!.addEventListener("click", run);
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") run();
  });
}

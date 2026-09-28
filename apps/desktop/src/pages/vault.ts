import { api } from "../api";
import { openCredentialForm } from "../components/credentialForm";
import { escapeHtml, guardBusy } from "../domUtils";
import { removeCredential, state, subscribe, upsertCredential } from "../state";
import { toast } from "../toast";
import type { Credential } from "../types";

function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

function initials(name: string): string {
  return (
    name
      .replace(/[^\p{L}\p{N}\s]/gu, " ")
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0]?.toUpperCase() ?? "")
      .join("") || "?"
  );
}

function timeAgo(iso: string | null): string {
  if (!iso) return "Never used";
  const diffMs = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diffMs / 60000);
  if (mins < 1) return "Just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(iso).toLocaleDateString();
}

export function renderVault(root: HTMLElement, opts: { favoritesOnly?: boolean } = {}) {
  let query = "";
  let unsub: (() => void) | null = null;

  root.innerHTML = `
    <div class="kf-page-header">
      <div>
        <h1>${opts.favoritesOnly ? "Favorites" : "Vault"}</h1>
        <p>${opts.favoritesOnly ? "Your starred logins." : "All of your saved logins, in one place."}</p>
      </div>
      <button class="kf-btn kf-btn-primary" id="add-btn">+ Add login</button>
    </div>
    <input class="kf-input kf-search-box" id="search" placeholder="Search by name, URL, or username…" />
    <div class="kf-cred-list" id="cred-list"></div>
  `;

  root.querySelector("#add-btn")!.addEventListener("click", () => openCredentialForm());
  const searchInput = root.querySelector<HTMLInputElement>("#search")!;
  searchInput.addEventListener("input", () => {
    query = searchInput.value.toLowerCase();
    draw();
  });

  function draw() {
    const listEl = root.querySelector<HTMLElement>("#cred-list")!;
    let items = state.credentials;
    if (opts.favoritesOnly) items = items.filter((c) => c.favorite);
    if (query) {
      items = items.filter(
        (c) =>
          c.name.toLowerCase().includes(query) ||
          c.url.toLowerCase().includes(query) ||
          c.username.toLowerCase().includes(query)
      );
    }
    items = [...items].sort((a, b) => a.name.localeCompare(b.name));

    if (items.length === 0) {
      listEl.innerHTML = `
        <div class="kf-empty-state">
          <div class="kf-logo-mark" style="margin:0 auto 16px">🔑</div>
          <p>${query ? "No logins match your search." : opts.favoritesOnly ? "No favorites yet — star a login to pin it here." : "No logins yet. Add your first one to get started."}</p>
        </div>`;
      return;
    }

    listEl.innerHTML = items.map(rowHtml).join("");

    for (const cred of items) {
      const row = listEl.querySelector<HTMLElement>(`[data-id="${cred.id}"]`)!;
      const starBtn = row.querySelector<HTMLButtonElement>(".kf-star")!;
      starBtn.addEventListener(
        "click",
        guardBusy(starBtn, async (e: MouseEvent) => {
          e.stopPropagation();
          // Without the guard, two rapid clicks both read the same
          // pre-toggle `cred.favorite` from this closure (captured at
          // the last draw()) and send the same target value twice —
          // harmless in outcome, but two redundant vault writes and
          // re-renders for one click's worth of user intent.
          const updated = await api.updateCredential(cred.id, { ...toInput(cred), favorite: !cred.favorite });
          upsertCredential(updated);
          draw();
        })
      );
      row.querySelector<HTMLElement>(".kf-copy-user")!.addEventListener("click", async (e) => {
        e.stopPropagation();
        await api.copyWithClipboardTimeout(cred.username, state.clipboardClearSeconds);
        toast(`Username copied — clears in ${state.clipboardClearSeconds}s.`);
      });
      row.querySelector<HTMLElement>(".kf-copy-pass")!.addEventListener("click", async (e) => {
        e.stopPropagation();
        await api.copyWithClipboardTimeout(cred.password, state.clipboardClearSeconds);
        await api.touchCredentialUsed(cred.id);
        toast(`Password copied — clears in ${state.clipboardClearSeconds}s.`);
      });
      const deleteBtn = row.querySelector<HTMLButtonElement>(".kf-delete")!;
      deleteBtn.addEventListener(
        "click",
        guardBusy(deleteBtn, async (e: MouseEvent) => {
          e.stopPropagation();
          if (!confirm(`Delete "${cred.name}"? This can't be undone.`)) return;
          await api.deleteCredential(cred.id);
          removeCredential(cred.id);
          draw();
        })
      );
      row.addEventListener("click", () => openCredentialForm(cred));
    }
  }

  function rowHtml(c: Credential): string {
    const badge =
      c.strength.band === "Very weak" || c.strength.band === "Weak"
        ? `<span class="kf-badge kf-badge-danger">weak</span>`
        : "";
    return `
      <div class="kf-cred-row" data-id="${c.id}">
        <div class="kf-cred-avatar">${initials(c.name)}</div>
        <div class="kf-cred-main">
          <div class="kf-cred-name">${escapeHtml(c.name)} ${badge}</div>
          <div class="kf-cred-sub">${escapeHtml(c.username || hostOf(c.url))} · ${timeAgo(c.last_used_at)}</div>
        </div>
        <div class="kf-cred-actions">
          <button class="kf-star ${c.favorite ? "active" : ""}" title="Favorite" aria-label="Toggle favorite">★</button>
          <button class="kf-icon-btn kf-copy-user" title="Copy username" aria-label="Copy username">👤</button>
          <button class="kf-icon-btn kf-copy-pass" title="Copy password" aria-label="Copy password">🔑</button>
          <button class="kf-icon-btn kf-delete" title="Delete" aria-label="Delete">🗑</button>
        </div>
      </div>
    `;
  }

  draw();
  unsub = subscribe(draw);
  return () => unsub?.();
}

function toInput(c: Credential) {
  return { name: c.name, url: c.url, username: c.username, password: c.password, notes: c.notes, tags: c.tags, favorite: c.favorite };
}

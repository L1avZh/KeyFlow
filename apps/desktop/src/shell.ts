import { api } from "./api";
import { renderGenerator } from "./pages/generator";
import { renderHome } from "./pages/home";
import { renderSecurity } from "./pages/security";
import { renderSettings } from "./pages/settings";
import { renderVault } from "./pages/vault";
import { Route, setRoute, state, subscribe } from "./state";

const NAV: { route: Route; label: string; icon: string }[] = [
  { route: "home", label: "Home", icon: "🏠" },
  { route: "vault", label: "Vault", icon: "🔒" },
  { route: "favorites", label: "Favorites", icon: "★" },
  { route: "generator", label: "Generator", icon: "🎲" },
  { route: "security", label: "Security", icon: "🛡" },
  { route: "settings", label: "Settings", icon: "⚙" },
];

export function renderShell(root: HTMLElement, onLocked: () => void) {
  root.innerHTML = `
    <div class="kf-app-shell">
      <nav class="kf-sidebar">
        <div class="kf-titlebar-spacer"></div>
        <div class="kf-brand"><div class="kf-logo-mark">🔑</div> KeyFlow</div>
        <div class="kf-nav" id="nav"></div>
        <div class="kf-sidebar-footer">
          <button class="kf-btn kf-btn-ghost kf-btn-sm" id="lock-btn">🔒 Lock now</button>
        </div>
      </nav>
      <main class="kf-main" id="main"></main>
    </div>
  `;

  const navEl = root.querySelector<HTMLElement>("#nav")!;
  const mainEl = root.querySelector<HTMLElement>("#main")!;

  function drawNav() {
    navEl.innerHTML = NAV.map(
      (n) => `<button class="kf-nav-item ${state.route === n.route ? "active" : ""}" data-route="${n.route}">
        <span class="kf-nav-icon">${n.icon}</span> ${n.label}
      </button>`
    ).join("");
    navEl.querySelectorAll<HTMLButtonElement>("[data-route]").forEach((btn) => {
      btn.addEventListener("click", () => setRoute(btn.dataset.route as Route));
    });
  }

  let cleanupPage: (() => void) | void;

  function drawPage() {
    cleanupPage?.();
    cleanupPage = undefined;
    switch (state.route) {
      case "home":
        cleanupPage = renderHome(mainEl, (r: Route) => setRoute(r));
        break;
      case "vault":
        cleanupPage = renderVault(mainEl);
        break;
      case "favorites":
        cleanupPage = renderVault(mainEl, { favoritesOnly: true });
        break;
      case "generator":
        renderGenerator(mainEl);
        break;
      case "security":
        renderSecurity(mainEl);
        break;
      case "settings":
        renderSettings(mainEl, onLocked);
        break;
    }
  }

  root.querySelector("#lock-btn")!.addEventListener("click", async () => {
    await api.lockVault();
    onLocked();
  });

  let lastRoute = state.route;
  drawNav();
  drawPage();
  return subscribe(() => {
    drawNav();
    // Only remount the page body on an actual route change — page
    // modules subscribe to data changes (new/edited credentials, etc.)
    // themselves and redraw just their own content, so a full remount
    // here would blow away in-progress search text or form state for
    // no reason.
    if (state.route !== lastRoute) {
      lastRoute = state.route;
      drawPage();
    }
  });
}

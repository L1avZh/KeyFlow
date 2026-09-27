import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import { initCommandPalette, triggerPalette } from "./components/commandPalette";
import { renderOnboarding } from "./pages/onboarding";
import { renderUnlock } from "./pages/unlock";
import { renderShell } from "./shell";
import { applyTheme, setCredentials, state } from "./state";

const root = document.getElementById("app")!;

let shellUnsub: (() => void) | void;
let inactivityTimer: number | undefined;

function resetInactivityTimer() {
  window.clearTimeout(inactivityTimer);
  if (state.autoLockMinutes <= 0) return;
  inactivityTimer = window.setTimeout(async () => {
    await api.lockVault();
    showLocked();
  }, state.autoLockMinutes * 60_000);
}

function wireActivityTracking() {
  ["mousedown", "keydown", "wheel", "touchstart"].forEach((evt) =>
    document.addEventListener(evt, resetInactivityTimer, { passive: true })
  );
  resetInactivityTimer();
}

async function showShell() {
  shellUnsub?.();
  shellUnsub = renderShell(root, showLocked);
  wireActivityTracking();
}

function showLocked() {
  window.clearTimeout(inactivityTimer);
  shellUnsub?.();
  shellUnsub = undefined;
  renderUnlock(root, showShell);
  attemptQuickUnlock();
}

async function attemptQuickUnlock() {
  try {
    const available = await api.quickUnlockAvailable();
    if (!available) return;
    const creds = await api.tryQuickUnlock();
    if (creds) {
      setCredentials(creds);
      showShell();
    }
  } catch {
    // Fall through to manual unlock — quick unlock is best-effort.
  }
}

async function bootstrap() {
  applyTheme();
  initCommandPalette(() => {
    /* route already updated via state; shell handles its own redraw */
  });

  // Best-effort: wiring the global-shortcut event listener must never
  // block (or blank-screen) the rest of the app if it fails for any
  // reason — onboarding/unlock/vault all work fine without it.
  listen("quick-access:toggle", () => {
    if (document.getElementById("app")?.querySelector(".kf-app-shell")) {
      triggerPalette();
    }
  }).catch(() => {});

  const exists = await api.vaultExists();
  if (!exists) {
    renderOnboarding(root, async () => {
      const creds = await api.listCredentials();
      setCredentials(creds);
      showShell();
    });
    return;
  }

  const unlocked = await api.isUnlocked();
  if (unlocked) {
    const creds = await api.listCredentials();
    setCredentials(creds);
    showShell();
    return;
  }

  showLocked();
}

bootstrap();

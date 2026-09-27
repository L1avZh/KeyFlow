import { api, friendlyError } from "../api";
import { setCredentials } from "../state";

export function renderUnlock(root: HTMLElement, onDone: () => void) {
  root.innerHTML = `
    <div class="kf-centered-shell">
      <div class="kf-auth-card">
        <div class="kf-logo-mark">🔑</div>
        <h1>KeyFlow is locked</h1>
        <p class="kf-subtitle">Enter your master password to unlock your vault.</p>
        <form id="unlock-form">
          <div class="kf-field">
            <label for="pw">Master password</label>
            <input id="pw" class="kf-input" type="password" autocomplete="current-password" required />
          </div>
          <button type="submit" class="kf-btn kf-btn-primary kf-btn-block" id="unlock-btn">Unlock</button>
          <div class="kf-error-text" id="unlock-error"></div>
        </form>
      </div>
    </div>
  `;

  const pw = root.querySelector<HTMLInputElement>("#pw")!;
  const form = root.querySelector<HTMLFormElement>("#unlock-form")!;
  const errorEl = root.querySelector<HTMLElement>("#unlock-error")!;
  const btn = root.querySelector<HTMLButtonElement>("#unlock-btn")!;

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    errorEl.textContent = "";
    btn.disabled = true;
    btn.textContent = "Unlocking…";
    try {
      const creds = await api.unlockVault(pw.value);
      setCredentials(creds);
      onDone();
    } catch (err) {
      errorEl.textContent = friendlyError(err);
      btn.disabled = false;
      btn.textContent = "Unlock";
      pw.value = "";
      pw.focus();
    }
  });

  pw.focus();
}

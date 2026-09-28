import { api, friendlyError } from "../api";
import { codepointLength } from "../domUtils";
import { toast } from "../toast";

const MIN_LENGTH = 10;

export function renderOnboarding(root: HTMLElement, onDone: () => void) {
  root.innerHTML = `
    <div class="kf-centered-shell">
      <div class="kf-auth-card">
        <div class="kf-logo-mark">🔑</div>
        <h1>Welcome to KeyFlow</h1>
        <p class="kf-subtitle">Your credentials stay protected by your master password — encrypted on this device, never sent anywhere.</p>
        <form id="onboard-form">
          <div class="kf-field">
            <label for="pw1">Create master password</label>
            <input id="pw1" class="kf-input" type="password" autocomplete="new-password" required minlength="${MIN_LENGTH}" />
          </div>
          <div class="kf-strength" id="strength-wrap" style="display:none">
            <div class="kf-strength-bar"><div class="kf-strength-fill" id="strength-fill"></div></div>
            <div class="kf-strength-label"><span id="strength-text"></span><span id="strength-bits"></span></div>
          </div>
          <div class="kf-field" style="margin-top:14px">
            <label for="pw2">Confirm master password</label>
            <input id="pw2" class="kf-input" type="password" autocomplete="new-password" required minlength="${MIN_LENGTH}" />
          </div>
          <p class="kf-hint" style="text-align:left; margin-bottom:14px">
            KeyFlow cannot recover this password for you — it isn't stored anywhere, even by us.
            If you forget it, your saved logins can't be decrypted. Consider writing it down somewhere safe.
          </p>
          <button type="submit" class="kf-btn kf-btn-primary kf-btn-block">Create Master Password</button>
          <div class="kf-error-text" id="onboard-error"></div>
        </form>
      </div>
    </div>
  `;

  const pw1 = root.querySelector<HTMLInputElement>("#pw1")!;
  const pw2 = root.querySelector<HTMLInputElement>("#pw2")!;
  const form = root.querySelector<HTMLFormElement>("#onboard-form")!;
  const errorEl = root.querySelector<HTMLElement>("#onboard-error")!;
  const strengthWrap = root.querySelector<HTMLElement>("#strength-wrap")!;
  const strengthFill = root.querySelector<HTMLElement>("#strength-fill")!;
  const strengthText = root.querySelector<HTMLElement>("#strength-text")!;
  const strengthBits = root.querySelector<HTMLElement>("#strength-bits")!;

  let debounce: number | undefined;
  pw1.addEventListener("input", () => {
    if (!pw1.value) {
      strengthWrap.style.display = "none";
      return;
    }
    strengthWrap.style.display = "block";
    window.clearTimeout(debounce);
    debounce = window.setTimeout(async () => {
      const s = await api.estimateStrength(pw1.value);
      const pct = Math.min(100, (s.entropy_bits / 100) * 100);
      const colors: Record<string, string> = {
        "Very weak": "var(--kf-danger)",
        Weak: "var(--kf-danger)",
        Fair: "var(--kf-warning)",
        Strong: "var(--kf-success)",
        "Very strong": "var(--kf-success)",
      };
      strengthFill.style.width = `${pct}%`;
      strengthFill.style.background = colors[s.band] ?? "var(--kf-text-muted)";
      strengthText.textContent = s.band;
      strengthBits.textContent = `${Math.round(s.entropy_bits)} bits`;
    }, 150);
  });

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    errorEl.textContent = "";
    if (pw1.value !== pw2.value) {
      errorEl.textContent = "Those passwords don't match.";
      return;
    }
    if (codepointLength(pw1.value) < MIN_LENGTH) {
      errorEl.textContent = `Use at least ${MIN_LENGTH} characters.`;
      return;
    }
    try {
      await api.createVault(pw1.value);
      toast("Vault created. Welcome to KeyFlow.");
      onDone();
    } catch (err) {
      errorEl.textContent = friendlyError(err);
    }
  });

  pw1.focus();
}

import { api, friendlyError } from "../api";
import { escapeHtml } from "../domUtils";
import { upsertCredential } from "../state";
import { toast } from "../toast";
import type { Credential } from "../types";
import { generatorDefaults, renderStrengthMeter } from "../pages/generator";

export function openCredentialForm(existing?: Credential) {
  const backdrop = document.createElement("div");
  backdrop.className = "kf-modal-backdrop";
  backdrop.innerHTML = `
    <div class="kf-modal" role="dialog" aria-modal="true" aria-labelledby="cred-form-title">
      <h2 id="cred-form-title">${existing ? "Edit login" : "Add login"}</h2>
      <form id="cred-form">
        <div class="kf-field">
          <label for="f-name">Name</label>
          <input id="f-name" class="kf-input" required placeholder="e.g. GitHub" value="${escapeHtml(existing?.name ?? "")}" />
        </div>
        <div class="kf-field">
          <label for="f-url">Website URL</label>
          <input id="f-url" class="kf-input" required placeholder="https://example.com" value="${escapeHtml(existing?.url ?? "")}" />
        </div>
        <div class="kf-field">
          <label for="f-username">Username or email</label>
          <input id="f-username" class="kf-input" placeholder="you@example.com" value="${escapeHtml(existing?.username ?? "")}" />
        </div>
        <div class="kf-field">
          <label for="f-password">Password</label>
          <div class="kf-row-gap">
            <input id="f-password" class="kf-input kf-input-mono kf-flex-1" type="text" value="${escapeHtml(existing?.password ?? "")}" />
            <button type="button" class="kf-btn kf-btn-sm" id="f-generate">Generate</button>
          </div>
          <div id="f-strength"></div>
        </div>
        <div class="kf-field">
          <label for="f-tags">Tags (comma separated)</label>
          <input id="f-tags" class="kf-input" placeholder="work, personal" value="${escapeHtml((existing?.tags ?? []).join(", "))}" />
        </div>
        <div class="kf-field">
          <label for="f-notes">Notes</label>
          <textarea id="f-notes" class="kf-textarea">${escapeHtml(existing?.notes ?? "")}</textarea>
        </div>
        <label class="kf-checkbox-row">
          <input type="checkbox" id="f-favorite" ${existing?.favorite ? "checked" : ""} />
          Favorite
        </label>
        <div class="kf-error-text" id="f-error"></div>
        <div class="kf-modal-actions">
          <button type="button" class="kf-btn" id="f-cancel">Cancel</button>
          <button type="submit" class="kf-btn kf-btn-primary">${existing ? "Save changes" : "Add login"}</button>
        </div>
      </form>
    </div>
  `;
  document.body.appendChild(backdrop);

  const close = () => backdrop.remove();
  backdrop.addEventListener("click", (e) => {
    if (e.target === backdrop) close();
  });
  backdrop.querySelector("#f-cancel")!.addEventListener("click", close);

  const passwordInput = backdrop.querySelector<HTMLInputElement>("#f-password")!;
  const strengthContainer = backdrop.querySelector<HTMLElement>("#f-strength")!;

  const refreshStrength = async () => {
    if (!passwordInput.value) {
      strengthContainer.innerHTML = "";
      return;
    }
    const s = await api.estimateStrength(passwordInput.value);
    strengthContainer.innerHTML = renderStrengthMeter(s);
  };
  passwordInput.addEventListener("input", refreshStrength);
  refreshStrength();

  backdrop.querySelector("#f-generate")!.addEventListener("click", async () => {
    const secret = await api.generatePassword(generatorDefaults());
    passwordInput.value = secret.value;
    refreshStrength();
  });

  const form = backdrop.querySelector<HTMLFormElement>("#cred-form")!;
  const errorEl = backdrop.querySelector<HTMLElement>("#f-error")!;
  const submitBtn = backdrop.querySelector<HTMLButtonElement>('button[type="submit"]')!;

  // A plain `async` submit handler with no guard let a fast double-click
  // (or double-Enter) fire two add_credential/update_credential calls
  // before the first IPC round-trip resolved — for "Add login" that's a
  // literal duplicate credential written to the vault, not just a UI
  // glitch. Disabling the submit button blocks both a second click *and*
  // implicit form-submission via Enter while it's disabled.
  let submitting = false;

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    if (submitting) return;
    submitting = true;
    submitBtn.disabled = true;
    errorEl.textContent = "";
    const input = {
      name: (backdrop.querySelector<HTMLInputElement>("#f-name")!).value.trim(),
      url: (backdrop.querySelector<HTMLInputElement>("#f-url")!).value.trim(),
      username: (backdrop.querySelector<HTMLInputElement>("#f-username")!).value.trim(),
      password: passwordInput.value,
      notes: (backdrop.querySelector<HTMLTextAreaElement>("#f-notes")!).value,
      tags: (backdrop.querySelector<HTMLInputElement>("#f-tags")!)
        .value.split(",")
        .map((t) => t.trim())
        .filter(Boolean),
      favorite: (backdrop.querySelector<HTMLInputElement>("#f-favorite")!).checked,
    };
    if (!/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(input.url)) {
      input.url = `https://${input.url}`;
    }
    try {
      const saved = existing
        ? await api.updateCredential(existing.id, input)
        : await api.addCredential(input);
      upsertCredential(saved);
      toast(existing ? "Login updated." : "Login saved.");
      close();
    } catch (err) {
      errorEl.textContent = friendlyError(err);
      submitting = false;
      submitBtn.disabled = false;
    }
  });
}

import { api, friendlyError } from "../api";
import { setAutoLockMinutes, setClipboardClearSeconds, setCredentials, setTheme, state } from "../state";
import { toast } from "../toast";

export function renderSettings(root: HTMLElement, onLocked: () => void) {
  root.innerHTML = `
    <div class="kf-page-header"><div><h1>Settings</h1><p>Manage your vault, security, and preferences.</p></div></div>

    <div class="kf-card" style="padding:20px; max-width:600px; margin-bottom:16px">
      <h2 style="font-size:14px; margin:0 0 12px">Appearance</h2>
      <div class="kf-field">
        <label for="s-theme">Theme</label>
        <select id="s-theme" class="kf-select">
          <option value="system">Match system</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </div>
    </div>

    <div class="kf-card" style="padding:20px; max-width:600px; margin-bottom:16px">
      <h2 style="font-size:14px; margin:0 0 12px">Locking</h2>
      <div class="kf-field">
        <label for="s-autolock">Auto-lock after inactivity (minutes, 0 = never)</label>
        <input id="s-autolock" class="kf-input" type="number" min="0" max="120" value="${state.autoLockMinutes}" />
      </div>
      <div class="kf-field">
        <label for="s-clip">Clear clipboard after copying a secret (seconds)</label>
        <input id="s-clip" class="kf-input" type="number" min="5" max="120" value="${state.clipboardClearSeconds}" />
      </div>
      <button class="kf-btn" id="s-lock-now">Lock KeyFlow now</button>
    </div>

    <div class="kf-card" style="padding:20px; max-width:600px; margin-bottom:16px">
      <h2 style="font-size:14px; margin:0 0 4px">Quick unlock</h2>
      <p class="kf-hint" style="margin-bottom:12px">
        Stores your master password in this OS's secure storage (Keychain / Credential Manager / Secret Service)
        so you don't have to retype it every launch. Anyone able to log into your OS user account could then
        unlock KeyFlow too — this trades a little security for convenience, and is off by default.
      </p>
      <label class="kf-checkbox-row"><input type="checkbox" id="s-quick-unlock" /> Enable quick unlock</label>
      <div id="s-quick-unlock-confirm" style="display:none; margin-top:10px">
        <div class="kf-field">
          <label for="s-quick-pw">Confirm master password</label>
          <input id="s-quick-pw" class="kf-input" type="password" />
        </div>
        <button class="kf-btn kf-btn-primary kf-btn-sm" id="s-quick-confirm-btn">Confirm</button>
      </div>
    </div>

    <div class="kf-card" style="padding:20px; max-width:600px; margin-bottom:16px">
      <h2 style="font-size:14px; margin:0 0 12px">Change master password</h2>
      <div class="kf-field">
        <label for="s-newpw1">New master password</label>
        <input id="s-newpw1" class="kf-input" type="password" minlength="10" />
      </div>
      <div class="kf-field">
        <label for="s-newpw2">Confirm new master password</label>
        <input id="s-newpw2" class="kf-input" type="password" minlength="10" />
      </div>
      <button class="kf-btn kf-btn-primary" id="s-change-pw">Change master password</button>
      <div class="kf-error-text" id="s-pw-error"></div>
    </div>

    <div class="kf-card" style="padding:20px; max-width:600px; margin-bottom:16px">
      <h2 style="font-size:14px; margin:0 0 4px">Export</h2>
      <p class="kf-hint" style="margin-bottom:12px">
        ⚠️ Exported files contain your passwords in <strong>plain text</strong>. Store the file somewhere secure
        and delete it once you're done with it.
      </p>
      <button class="kf-btn kf-btn-danger" id="s-export">Export vault as JSON…</button>
    </div>

    <div class="kf-card" style="padding:20px; max-width:600px">
      <h2 style="font-size:14px; margin:0 0 4px">Import</h2>
      <p class="kf-hint" style="margin-bottom:12px">CSV with columns: name, url, username, password, notes.</p>
      <button class="kf-btn" id="s-import">Choose CSV file…</button>
      <div id="s-import-preview" style="margin-top:14px"></div>
    </div>
  `;

  const themeSelect = root.querySelector<HTMLSelectElement>("#s-theme")!;
  themeSelect.value = state.theme;
  themeSelect.addEventListener("change", () => setTheme(themeSelect.value as any));

  root.querySelector<HTMLInputElement>("#s-autolock")!.addEventListener("change", (e) => {
    setAutoLockMinutes(Number((e.target as HTMLInputElement).value));
    toast("Auto-lock timeout updated.");
  });
  root.querySelector<HTMLInputElement>("#s-clip")!.addEventListener("change", (e) => {
    setClipboardClearSeconds(Number((e.target as HTMLInputElement).value));
    toast("Clipboard timeout updated.");
  });
  root.querySelector("#s-lock-now")!.addEventListener("click", async () => {
    await api.lockVault();
    onLocked();
  });

  const quickToggle = root.querySelector<HTMLInputElement>("#s-quick-unlock")!;
  const quickConfirm = root.querySelector<HTMLElement>("#s-quick-unlock-confirm")!;
  api.quickUnlockAvailable().then((enabled) => (quickToggle.checked = enabled));
  quickToggle.addEventListener("change", async () => {
    if (quickToggle.checked) {
      quickConfirm.style.display = "block";
    } else {
      await api.setQuickUnlock(false);
      toast("Quick unlock disabled.");
    }
  });
  root.querySelector("#s-quick-confirm-btn")!.addEventListener("click", async () => {
    const pw = root.querySelector<HTMLInputElement>("#s-quick-pw")!.value;
    try {
      await api.setQuickUnlock(true, pw);
      quickConfirm.style.display = "none";
      toast("Quick unlock enabled.");
    } catch (err) {
      toast(friendlyError(err), "danger");
      quickToggle.checked = false;
    }
  });

  root.querySelector("#s-change-pw")!.addEventListener("click", async () => {
    const p1 = root.querySelector<HTMLInputElement>("#s-newpw1")!;
    const p2 = root.querySelector<HTMLInputElement>("#s-newpw2")!;
    const errorEl = root.querySelector<HTMLElement>("#s-pw-error")!;
    errorEl.textContent = "";
    if (p1.value.length < 10) {
      errorEl.textContent = "Use at least 10 characters.";
      return;
    }
    if (p1.value !== p2.value) {
      errorEl.textContent = "Those passwords don't match.";
      return;
    }
    try {
      await api.changeMasterPassword(p1.value);
      p1.value = "";
      p2.value = "";
      toast("Master password changed.");
    } catch (err) {
      errorEl.textContent = friendlyError(err);
    }
  });

  root.querySelector("#s-export")!.addEventListener("click", async () => {
    if (!confirm("This will save your passwords in plain text to a file. Continue?")) return;
    try {
      const saved = await api.exportJsonViaDialog();
      if (saved) toast("Exported. Remember to delete the file once you're done with it.");
    } catch (err) {
      toast(friendlyError(err), "danger");
    }
  });

  root.querySelector("#s-import")!.addEventListener("click", async () => {
    try {
      const text = await api.pickAndReadCsv();
      if (text === null) return;
      const preview = await api.previewCsvImport(text);
      renderImportPreview(root, preview, async () => {
        setCredentials(await api.listCredentials());
      });
    } catch (err) {
      toast(friendlyError(err), "danger");
    }
  });
}

function renderImportPreview(root: HTMLElement, preview: Awaited<ReturnType<typeof api.previewCsvImport>>, onCommitted: () => void) {
  const el = root.querySelector<HTMLElement>("#s-import-preview")!;
  const warningsHtml = preview.warnings.length
    ? `<ul style="margin:8px 0; padding-left:18px; font-size:12px; color:var(--kf-warning)">${preview.warnings
        .map((w) => `<li>Row ${w.row}: ${w.message}</li>`)
        .join("")}</ul>`
    : "";
  el.innerHTML = `
    <p><strong>${preview.credentials.length}</strong> logins ready to import.</p>
    ${warningsHtml}
    <button class="kf-btn kf-btn-primary kf-btn-sm" id="s-import-commit">Import ${preview.credentials.length} logins</button>
  `;
  el.querySelector("#s-import-commit")!.addEventListener("click", async () => {
    await api.commitImport(preview.credentials);
    toast("Import complete.");
    el.innerHTML = "";
    onCommitted();
  });
}

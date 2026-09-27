import { api } from "../api";
import { toast } from "../toast";
import type { PassphraseOptions, PasswordOptions, StrengthEstimate } from "../types";

export function generatorDefaults(): PasswordOptions {
  return { length: 20, uppercase: true, lowercase: true, digits: true, symbols: true, exclude_ambiguous: true };
}

function passphraseDefaults(): PassphraseOptions {
  return { word_count: 5, separator: "-", capitalize: true, include_number: true };
}

const strengthColor: Record<string, string> = {
  "Very weak": "var(--kf-danger)",
  Weak: "var(--kf-danger)",
  Fair: "var(--kf-warning)",
  Strong: "var(--kf-success)",
  "Very strong": "var(--kf-success)",
};

export function renderStrengthMeter(s: StrengthEstimate): string {
  const pct = Math.min(100, (s.entropy_bits / 100) * 100);
  return `
    <div class="kf-strength">
      <div class="kf-strength-bar"><div class="kf-strength-fill" style="width:${pct}%; background:${strengthColor[s.band] ?? "var(--kf-text-muted)"}"></div></div>
      <div class="kf-strength-label"><span>${s.band}</span><span>${Math.round(s.entropy_bits)} bits</span></div>
    </div>
  `;
}

export function renderGenerator(root: HTMLElement) {
  let mode: "password" | "passphrase" = "password";
  const pwOpts = generatorDefaults();
  const phraseOpts = passphraseDefaults();
  let current = "";

  root.innerHTML = `
    <div class="kf-page-header">
      <div><h1>Password Generator</h1><p>Create strong, unique passwords and passphrases.</p></div>
    </div>
    <div class="kf-card" style="padding:20px; max-width:560px">
      <div class="kf-tab-bar">
        <button class="kf-tab active" data-mode="password">Random Password</button>
        <button class="kf-tab" data-mode="passphrase">Passphrase</button>
      </div>

      <div class="kf-row-gap" style="margin-bottom:16px">
        <input id="gen-output" class="kf-input kf-input-mono kf-flex-1" readonly style="font-size:15px" />
        <button class="kf-icon-btn" id="gen-copy" title="Copy" aria-label="Copy">📋</button>
        <button class="kf-icon-btn" id="gen-refresh" title="Regenerate" aria-label="Regenerate">🔄</button>
      </div>
      <div id="gen-strength"></div>

      <div id="gen-options" style="margin-top:20px"></div>
    </div>
  `;

  const output = root.querySelector<HTMLInputElement>("#gen-output")!;
  const strengthEl = root.querySelector<HTMLElement>("#gen-strength")!;
  const optionsEl = root.querySelector<HTMLElement>("#gen-options")!;
  const tabs = root.querySelectorAll<HTMLButtonElement>(".kf-tab");

  async function regenerate() {
    const secret =
      mode === "password" ? await api.generatePassword(pwOpts) : await api.generatePassphrase(phraseOpts);
    current = secret.value;
    output.value = current;
    strengthEl.innerHTML = renderStrengthMeter(secret.strength);
  }

  function renderOptions() {
    if (mode === "password") {
      optionsEl.innerHTML = `
        <div class="kf-field">
          <label for="opt-length">Length: <span id="len-val">${pwOpts.length}</span></label>
          <input id="opt-length" type="range" min="8" max="64" value="${pwOpts.length}" style="width:100%" />
        </div>
        <label class="kf-checkbox-row"><input type="checkbox" id="opt-upper" ${pwOpts.uppercase ? "checked" : ""}/> Uppercase (A-Z)</label>
        <label class="kf-checkbox-row"><input type="checkbox" id="opt-lower" ${pwOpts.lowercase ? "checked" : ""}/> Lowercase (a-z)</label>
        <label class="kf-checkbox-row"><input type="checkbox" id="opt-digits" ${pwOpts.digits ? "checked" : ""}/> Numbers (0-9)</label>
        <label class="kf-checkbox-row"><input type="checkbox" id="opt-symbols" ${pwOpts.symbols ? "checked" : ""}/> Symbols (!@#$…)</label>
        <label class="kf-checkbox-row"><input type="checkbox" id="opt-ambiguous" ${pwOpts.exclude_ambiguous ? "checked" : ""}/> Exclude ambiguous characters (0/O, 1/l/I…)</label>
      `;
      optionsEl.querySelector("#opt-length")!.addEventListener("input", (e) => {
        pwOpts.length = Number((e.target as HTMLInputElement).value);
        optionsEl.querySelector("#len-val")!.textContent = String(pwOpts.length);
        regenerate();
      });
      const bind = (id: string, key: keyof PasswordOptions) => {
        optionsEl.querySelector(`#${id}`)!.addEventListener("change", (e) => {
          (pwOpts as any)[key] = (e.target as HTMLInputElement).checked;
          regenerate();
        });
      };
      bind("opt-upper", "uppercase");
      bind("opt-lower", "lowercase");
      bind("opt-digits", "digits");
      bind("opt-symbols", "symbols");
      bind("opt-ambiguous", "exclude_ambiguous");
    } else {
      optionsEl.innerHTML = `
        <div class="kf-field">
          <label for="opt-words">Words: <span id="words-val">${phraseOpts.word_count}</span></label>
          <input id="opt-words" type="range" min="3" max="10" value="${phraseOpts.word_count}" style="width:100%" />
        </div>
        <div class="kf-field">
          <label for="opt-sep">Separator</label>
          <input id="opt-sep" class="kf-input" value="${phraseOpts.separator}" maxlength="3" style="width:80px" />
        </div>
        <label class="kf-checkbox-row"><input type="checkbox" id="opt-cap" ${phraseOpts.capitalize ? "checked" : ""}/> Capitalize words</label>
        <label class="kf-checkbox-row"><input type="checkbox" id="opt-num" ${phraseOpts.include_number ? "checked" : ""}/> Append a number</label>
      `;
      optionsEl.querySelector("#opt-words")!.addEventListener("input", (e) => {
        phraseOpts.word_count = Number((e.target as HTMLInputElement).value);
        optionsEl.querySelector("#words-val")!.textContent = String(phraseOpts.word_count);
        regenerate();
      });
      optionsEl.querySelector("#opt-sep")!.addEventListener("input", (e) => {
        phraseOpts.separator = (e.target as HTMLInputElement).value;
        regenerate();
      });
      optionsEl.querySelector("#opt-cap")!.addEventListener("change", (e) => {
        phraseOpts.capitalize = (e.target as HTMLInputElement).checked;
        regenerate();
      });
      optionsEl.querySelector("#opt-num")!.addEventListener("change", (e) => {
        phraseOpts.include_number = (e.target as HTMLInputElement).checked;
        regenerate();
      });
    }
  }

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
      tabs.forEach((t) => t.classList.remove("active"));
      tab.classList.add("active");
      mode = tab.dataset.mode as "password" | "passphrase";
      renderOptions();
      regenerate();
    });
  });

  root.querySelector("#gen-copy")!.addEventListener("click", async () => {
    await api.copyWithClipboardTimeout(current, 20);
    toast("Copied — clipboard clears in 20s.");
  });
  root.querySelector("#gen-refresh")!.addEventListener("click", regenerate);

  renderOptions();
  regenerate();
}

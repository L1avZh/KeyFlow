// Dev-only: fakes the window.__TAURI_INTERNALS__ bridge so src/ can be
// rendered and clicked through in a plain browser tab. Not shipped;
// see dev-preview.html.

function estimateStrengthMock(password: string) {
  const hasLower = /[a-z]/.test(password);
  const hasUpper = /[A-Z]/.test(password);
  const hasDigit = /[0-9]/.test(password);
  const hasSymbol = /[^a-zA-Z0-9]/.test(password);
  let alphabet = 0;
  if (hasLower) alphabet += 26;
  if (hasUpper) alphabet += 26;
  if (hasDigit) alphabet += 10;
  if (hasSymbol) alphabet += 25;
  const bits = password.length * Math.log2(alphabet || 1);
  let band = "Very weak";
  if (bits >= 80) band = "Very strong";
  else if (bits >= 60) band = "Strong";
  else if (bits >= 45) band = "Fair";
  else if (bits >= 28) band = "Weak";
  return { entropy_bits: bits, band };
}

function makeCred(overrides: Record<string, any>) {
  const base = {
    id: crypto.randomUUID(),
    name: "Example",
    url: "https://example.com",
    username: "user@example.com",
    password: "correct-horse-battery-staple-42",
    notes: "",
    tags: [],
    favorite: false,
    created_at: new Date(Date.now() - 90 * 86400000).toISOString(),
    updated_at: new Date(Date.now() - 90 * 86400000).toISOString(),
    last_used_at: new Date(Date.now() - 3600_000).toISOString(),
    password_age_days: 90,
  };
  const merged = { ...base, ...overrides };
  return { ...merged, strength: estimateStrengthMock(merged.password) };
}

let vaultCreds = [
  makeCred({ name: "GitHub", url: "https://github.com", username: "jordan@example.com", password: "Xk9#mQ2$pL7vR4nW!8zT", favorite: true }),
  makeCred({ name: "Gmail (personal)", url: "https://mail.google.com", username: "jordan@gmail.com", password: "abc123", tags: ["personal"] }),
  makeCred({ name: "Gmail (work)", url: "https://mail.google.com", username: "jordan@company.com", password: "Tr0ub4dor&3xtra!Long", tags: ["work"] }),
  makeCred({ name: "Old Forum", url: "https://forum.example.net", username: "jordan", password: "abc123", updated_at: new Date(Date.now() - 400 * 86400000).toISOString(), last_used_at: null }),
  makeCred({ name: "Stripe", url: "https://dashboard.stripe.com", username: "jordan@company.com", password: "9!vQxR2$kLp8mZ7wYt3B", favorite: true, last_used_at: null }),
];
let unlocked = false;

const handlers: Record<string, (args: any) => any> = {
  vault_exists: () => true,
  is_unlocked: () => unlocked,
  quick_unlock_available: () => false,
  create_vault: () => {
    unlocked = true;
  },
  unlock_vault: () => {
    unlocked = true;
    return vaultCreds;
  },
  try_quick_unlock: () => null,
  lock_vault: () => {
    unlocked = false;
  },
  list_credentials: () => vaultCreds,
  add_credential: (args: any) => {
    const c = makeCred({ ...args.input, created_at: new Date().toISOString(), updated_at: new Date().toISOString(), last_used_at: null });
    vaultCreds.push(c);
    return c;
  },
  update_credential: (args: any) => {
    const idx = vaultCreds.findIndex((c: any) => c.id === args.id);
    const updated = makeCred({ ...vaultCreds[idx], ...args.input, id: args.id, updated_at: new Date().toISOString() });
    vaultCreds[idx] = updated;
    return updated;
  },
  delete_credential: (args: any) => {
    vaultCreds = vaultCreds.filter((c: any) => c.id !== args.id);
  },
  touch_credential_used: (args: any) => {
    const c = vaultCreds.find((c: any) => c.id === args.id);
    if (c) c.last_used_at = new Date().toISOString();
  },
  find_autofill_matches: (args: any) => {
    try {
      const host = new URL(args.url).host;
      return vaultCreds
        .filter((c: any) => {
          try {
            return new URL(c.url).host === host;
          } catch {
            return false;
          }
        })
        .map((c: any) => ({ credential: c, decision: "exact" }));
    } catch {
      return [];
    }
  },
  explain_match: () => "no-match",
  generate_password_cmd: (args: any) => {
    const chars = "ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789!@#$%^&*";
    let out = "";
    for (let i = 0; i < args.opts.length; i++) out += chars[Math.floor(Math.random() * chars.length)];
    return { value: out, strength: estimateStrengthMock(out) };
  },
  generate_passphrase_cmd: (args: any) => {
    const words = ["orbit", "maple", "copper", "willow", "denim", "harbor", "quartz", "ember", "cider", "trail"];
    const parts = [];
    for (let i = 0; i < args.opts.word_count; i++) parts.push(words[Math.floor(Math.random() * words.length)]);
    const val = parts.join(args.opts.separator || "-");
    return { value: val, strength: estimateStrengthMock(val) };
  },
  estimate_strength_cmd: (args: any) => estimateStrengthMock(args.password),
  security_overview: () => ({
    total: vaultCreds.length,
    weak: vaultCreds.filter((c: any) => ["Very weak", "Weak"].includes(estimateStrengthMock(c.password).band)).length,
    reused: 2,
    old: 1,
    duplicates: 0,
    missing_password: 0,
  }),
  weak_credentials: () => vaultCreds.filter((c: any) => ["Very weak", "Weak"].includes(estimateStrengthMock(c.password).band)),
  old_credentials: () => vaultCreds.filter((c: any) => c.password_age_days > 180),
  reused_credential_groups: () => {
    const pw = "abc123";
    const group = vaultCreds.filter((c: any) => c.password === pw);
    return group.length > 1 ? [group] : [];
  },
  duplicate_credential_groups: () => [],
  change_master_password: () => {},
  set_quick_unlock: () => {},
  export_json: () => JSON.stringify(vaultCreds, null, 2),
  export_json_to_path: () => {},
  read_text_file: () => "name,url,username,password,notes\n",
  preview_csv_import: () => ({ credentials: [], warnings: [] }),
  commit_import: () => {},
  copy_with_clipboard_timeout: () => {},
};

(window as any).__TAURI_INTERNALS__ = {
  transformCallback(_callback: any) {
    return Math.floor(Math.random() * 1e9);
  },
  invoke(cmd: string, args: any) {
    if (cmd.startsWith("plugin:event|")) return Promise.resolve(0);
    const handler = handlers[cmd];
    if (!handler) {
      console.warn("[dev-preview] unhandled mock command", cmd, args);
      return Promise.resolve(undefined);
    }
    try {
      return Promise.resolve(handler(args));
    } catch (e) {
      return Promise.reject(String(e));
    }
  },
};

/**
 * Wraps an async click handler so a second click can't re-enter it while
 * the first invocation is still in flight. Every async button handler in
 * this app used to be a plain `async () => { await api.foo(); ... }`
 * with nothing stopping a fast double-click (or double-Enter on a form)
 * from firing the handler twice before the first IPC round-trip
 * resolved. For most buttons that's just a wasted duplicate request, but
 * for "Add login" and "Import N logins" it meant a literal duplicate
 * credential written to the vault — found by reasoning through the
 * actual async timing, not by running into it interactively.
 *
 * Disables the button for the duration of the call and always
 * re-enables it afterward, even if the button has since been removed
 * from the DOM (where `.disabled = false` is a harmless no-op).
 */
export function guardBusy<T extends HTMLButtonElement, E extends Event>(button: T, fn: (e: E) => Promise<void>) {
  return async (e: E) => {
    if (button.disabled) return;
    button.disabled = true;
    try {
      await fn(e);
    } finally {
      button.disabled = false;
    }
  };
}

/**
 * Counts Unicode codepoints the way Rust's `.chars().count()` does —
 * unlike JS's native `string.length`, which counts UTF-16 code units.
 * They diverge for any character outside the Basic Multilingual Plane
 * (most emoji): a 5-emoji password shows `.length === 10` (each emoji is
 * a 2-unit surrogate pair) while Rust sees 5 characters. The backend's
 * master-password minimum is enforced with `.chars().count()` (see
 * `keyflow-core::vault::validate_master_password`); using plain
 * `.length` here let a password pass this UI's own check and then get
 * rejected by the backend with a confusing "must be at least 10
 * characters" error for something that looked plenty long — confirmed
 * by directly comparing `"🎉🎉🎉🎉🎉".length` (10) against Rust's
 * `.chars().count()` (5) for the same string, not assumed.
 */
export function codepointLength(s: string): number {
  return [...s].length;
}

/**
 * Escapes text for safe interpolation into `innerHTML`. Every credential
 * field (name, username, url, notes) is attacker-controllable — imported
 * from a CSV file, or just typed by whoever set up a phishing-style
 * "credential" — and several pages render these fields via template
 * strings assigned to `innerHTML`.
 *
 * This used to be reimplemented privately, inconsistently, in three
 * separate files (vault.ts, credentialForm.ts, commandPalette.ts), and
 * two more pages that render credential fields the same way — home.ts's
 * "Recently used" list and security.ts's weak/old/reused/duplicates
 * lists and Autofill Tester results — had no escaping at all, a genuine
 * stored-XSS-shaped gap found by grepping every `innerHTML` assignment
 * in the app for which ones interpolate credential data, not by
 * assuming the existing per-file copies were applied everywhere they
 * needed to be. KeyFlow's CSP (`default-src 'self'`, no `unsafe-inline`
 * for scripts) happens to block inline `<script>`/`onerror=...` from
 * actually executing today, but relying on CSP alone instead of
 * escaping output is not a substitute for doing this correctly, and a
 * future CSP change (or a browser bug) shouldn't be what stands between
 * an unescaped credential name and script execution in the same
 * privileged webview that has full Tauri IPC access to the vault.
 */
export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));
}

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

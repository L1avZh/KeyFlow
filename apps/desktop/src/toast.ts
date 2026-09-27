let stack: HTMLElement | null = null;

function ensureStack(): HTMLElement {
  if (!stack) {
    stack = document.createElement("div");
    stack.className = "kf-toast-stack";
    stack.setAttribute("role", "status");
    stack.setAttribute("aria-live", "polite");
    document.body.appendChild(stack);
  }
  return stack;
}

export function toast(message: string, kind: "info" | "danger" = "info", ms = 3200) {
  const el = document.createElement("div");
  el.className = kind === "danger" ? "kf-toast kf-toast-danger" : "kf-toast";
  el.textContent = message;
  ensureStack().appendChild(el);
  setTimeout(() => el.remove(), ms);
}

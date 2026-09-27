import type { Credential } from "./types";

export type Route = "home" | "vault" | "favorites" | "generator" | "security" | "settings";

interface AppState {
  credentials: Credential[];
  route: Route;
  theme: "system" | "light" | "dark";
  autoLockMinutes: number;
  clipboardClearSeconds: number;
}

const listeners = new Set<() => void>();

export const state: AppState = {
  credentials: [],
  route: "home",
  theme: (localStorage.getItem("kf-theme") as AppState["theme"]) || "system",
  autoLockMinutes: Number(localStorage.getItem("kf-auto-lock-minutes") ?? 10),
  clipboardClearSeconds: Number(localStorage.getItem("kf-clipboard-seconds") ?? 20),
};

export function subscribe(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function notify() {
  for (const fn of listeners) fn();
}

export function setCredentials(creds: Credential[]) {
  state.credentials = creds;
  notify();
}

export function upsertCredential(cred: Credential) {
  const idx = state.credentials.findIndex((c) => c.id === cred.id);
  if (idx >= 0) state.credentials[idx] = cred;
  else state.credentials.push(cred);
  notify();
}

export function removeCredential(id: string) {
  state.credentials = state.credentials.filter((c) => c.id !== id);
  notify();
}

export function setRoute(route: Route) {
  state.route = route;
  notify();
}

export function setTheme(theme: AppState["theme"]) {
  state.theme = theme;
  localStorage.setItem("kf-theme", theme);
  applyTheme();
  notify();
}

export function applyTheme() {
  const root = document.documentElement;
  if (state.theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", state.theme);
}

export function setAutoLockMinutes(minutes: number) {
  state.autoLockMinutes = minutes;
  localStorage.setItem("kf-auto-lock-minutes", String(minutes));
  notify();
}

export function setClipboardClearSeconds(seconds: number) {
  state.clipboardClearSeconds = seconds;
  localStorage.setItem("kf-clipboard-seconds", String(seconds));
  notify();
}

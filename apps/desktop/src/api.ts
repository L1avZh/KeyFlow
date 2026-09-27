import { invoke } from "@tauri-apps/api/core";
import type {
  AutofillMatch,
  Credential,
  CredentialInput,
  GeneratedSecret,
  ImportPreview,
  PassphraseOptions,
  PasswordOptions,
  SecurityOverview,
  StrengthEstimate,
} from "./types";

/** Thin, typed wrapper around every Tauri command KeyFlow exposes. */
export const api = {
  vaultExists: () => invoke<boolean>("vault_exists"),
  isUnlocked: () => invoke<boolean>("is_unlocked"),
  quickUnlockAvailable: () => invoke<boolean>("quick_unlock_available"),
  createVault: (masterPassword: string) => invoke<void>("create_vault", { masterPassword }),
  unlockVault: (masterPassword: string) => invoke<Credential[]>("unlock_vault", { masterPassword }),
  tryQuickUnlock: () => invoke<Credential[] | null>("try_quick_unlock"),
  lockVault: () => invoke<void>("lock_vault"),

  listCredentials: () => invoke<Credential[]>("list_credentials"),
  addCredential: (input: CredentialInput) => invoke<Credential>("add_credential", { input }),
  updateCredential: (id: string, input: CredentialInput) => invoke<Credential>("update_credential", { id, input }),
  deleteCredential: (id: string) => invoke<void>("delete_credential", { id }),
  touchCredentialUsed: (id: string) => invoke<void>("touch_credential_used", { id }),

  findAutofillMatches: (url: string) => invoke<AutofillMatch[]>("find_autofill_matches", { url }),
  explainMatch: (id: string, url: string) => invoke<string>("explain_match", { id, url }),

  generatePassword: (opts: PasswordOptions) => invoke<GeneratedSecret>("generate_password_cmd", { opts }),
  generatePassphrase: (opts: PassphraseOptions) => invoke<GeneratedSecret>("generate_passphrase_cmd", { opts }),
  estimateStrength: (password: string) => invoke<StrengthEstimate>("estimate_strength_cmd", { password }),

  securityOverview: () => invoke<SecurityOverview>("security_overview"),
  weakCredentials: () => invoke<Credential[]>("weak_credentials"),
  oldCredentials: () => invoke<Credential[]>("old_credentials"),
  reusedCredentialGroups: () => invoke<Credential[][]>("reused_credential_groups"),
  duplicateCredentialGroups: () => invoke<Credential[][]>("duplicate_credential_groups"),

  changeMasterPassword: (newPassword: string) => invoke<void>("change_master_password", { newPassword }),
  setQuickUnlock: (enabled: boolean, masterPassword?: string) =>
    invoke<void>("set_quick_unlock", { enabled, masterPassword: masterPassword ?? null }),

  exportJson: () => invoke<string>("export_json"),
  /** Opens the native save dialog in Rust and writes there directly; returns false if cancelled. */
  exportJsonViaDialog: () => invoke<boolean>("export_json_via_dialog"),
  /** Opens the native open dialog in Rust and reads the chosen CSV file directly; returns null if cancelled. */
  pickAndReadCsv: () => invoke<string | null>("pick_and_read_csv"),
  previewCsvImport: (csvText: string) => invoke<ImportPreview>("preview_csv_import", { csvText }),
  commitImport: (credentials: CredentialInput[]) => invoke<void>("commit_import", { credentials }),

  copyWithClipboardTimeout: (text: string, seconds: number) =>
    invoke<void>("copy_with_clipboard_timeout", { text, seconds }),
};

export function friendlyError(e: unknown): string {
  const raw = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
  if (raw.includes("incorrect master password")) return "That master password isn't right. Try again.";
  if (raw.includes("vault is locked")) return "KeyFlow is locked. Unlock it first.";
  if (raw.includes("corrupted")) return "This vault file looks corrupted and couldn't be read.";
  return raw;
}

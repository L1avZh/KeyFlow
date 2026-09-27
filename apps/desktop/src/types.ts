export interface StrengthEstimate {
  entropy_bits: number;
  band: "Very weak" | "Weak" | "Fair" | "Strong" | "Very strong";
}

export interface Credential {
  id: string;
  name: string;
  url: string;
  username: string;
  password: string;
  notes: string;
  tags: string[];
  favorite: boolean;
  created_at: string;
  updated_at: string;
  last_used_at: string | null;
  password_age_days: number;
  strength: StrengthEstimate;
}

export interface CredentialInput {
  name: string;
  url: string;
  username: string;
  password: string;
  notes: string;
  tags: string[];
  favorite: boolean;
}

export interface SecurityOverview {
  total: number;
  weak: number;
  reused: number;
  old: number;
  duplicates: number;
  missing_password: number;
}

export interface ImportWarning {
  row: number;
  message: string;
}

export interface ImportPreview {
  credentials: CredentialInput[];
  warnings: ImportWarning[];
}

export interface AutofillMatch {
  credential: Credential;
  decision: string;
}

export interface PasswordOptions {
  length: number;
  uppercase: boolean;
  lowercase: boolean;
  digits: boolean;
  symbols: boolean;
  exclude_ambiguous: boolean;
}

export interface PassphraseOptions {
  word_count: number;
  separator: string;
  capitalize: boolean;
  include_number: boolean;
}

export interface GeneratedSecret {
  value: string;
  strength: StrengthEstimate;
}

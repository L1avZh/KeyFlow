//! Data-transfer objects passed across the Tauri IPC boundary.
//!
//! These deliberately mirror `keyflow-core` types rather than exposing
//! them directly, so the wire format (JSON, string IDs/timestamps) can
//! evolve independently of the core crate's internal representation.

use keyflow_core::credential::Credential;
use keyflow_core::domain::MatchDecision;
use keyflow_core::generator::{StrengthBand, StrengthEstimate};
use keyflow_core::vault::{ImportPreview, ImportWarning, SecurityOverview};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;

fn fmt_time(t: time::OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_else(|_| t.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialDto {
    pub id: String,
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
    pub password_age_days: i64,
    pub strength: StrengthEstimateDto,
}

impl From<&Credential> for CredentialDto {
    fn from(c: &Credential) -> Self {
        let strength = keyflow_core::generator::estimate_strength(&c.password);
        Self {
            id: c.id.to_string(),
            name: c.name.clone(),
            url: c.url.clone(),
            username: c.username.clone(),
            password: c.password.clone(),
            notes: c.notes.clone(),
            tags: c.tags.clone(),
            favorite: c.favorite,
            created_at: fmt_time(c.created_at),
            updated_at: fmt_time(c.updated_at),
            last_used_at: c.last_used_at.map(fmt_time),
            password_age_days: c.password_age_days(),
            strength: strength.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialInput {
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrengthEstimateDto {
    pub entropy_bits: f64,
    pub band: String,
}

impl From<StrengthEstimate> for StrengthEstimateDto {
    fn from(s: StrengthEstimate) -> Self {
        Self {
            entropy_bits: s.entropy_bits,
            band: band_label(s.band),
        }
    }
}

fn band_label(band: StrengthBand) -> String {
    band.label().to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct SecurityOverviewDto {
    pub total: usize,
    pub weak: usize,
    pub reused: usize,
    pub old: usize,
    pub duplicates: usize,
    pub missing_password: usize,
}

impl From<SecurityOverview> for SecurityOverviewDto {
    fn from(o: SecurityOverview) -> Self {
        Self {
            total: o.total,
            weak: o.weak,
            reused: o.reused,
            old: o.old,
            duplicates: o.duplicates,
            missing_password: o.missing_password,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportWarningDto {
    pub row: usize,
    pub message: String,
}

impl From<&ImportWarning> for ImportWarningDto {
    fn from(w: &ImportWarning) -> Self {
        Self { row: w.row, message: w.message.clone() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportPreviewDto {
    pub credentials: Vec<CredentialInput>,
    pub warnings: Vec<ImportWarningDto>,
}

impl From<ImportPreview> for ImportPreviewDto {
    fn from(p: ImportPreview) -> Self {
        Self {
            credentials: p
                .credentials
                .iter()
                .map(|c| CredentialInput {
                    name: c.name.clone(),
                    url: c.url.clone(),
                    username: c.username.clone(),
                    password: c.password.clone(),
                    notes: c.notes.clone(),
                    tags: c.tags.clone(),
                    favorite: c.favorite,
                })
                .collect(),
            warnings: p.warnings.iter().map(ImportWarningDto::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AutofillMatchDto {
    pub credential: CredentialDto,
    pub decision: String,
}

pub fn decision_label(decision: &MatchDecision) -> String {
    match decision {
        MatchDecision::ExactMatch => "exact".to_string(),
        MatchDecision::SubdomainMatch => "subdomain".to_string(),
        MatchDecision::Blocked { reason } => format!("blocked: {reason}"),
        MatchDecision::NoMatch => "no-match".to_string(),
    }
}

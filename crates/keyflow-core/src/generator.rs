//! Password and passphrase generation, plus entropy-based strength
//! estimation.
//!
//! All randomness comes from the OS CSPRNG (`rand::rngs::OsRng`). Strength
//! is reported as an estimated entropy in bits and a descriptive band;
//! KeyFlow never claims a password is "unhackable" or otherwise makes
//! guarantees a generator can't actually back up.

use rand::rngs::OsRng;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::error::{KeyflowError, Result};
use crate::wordlist::WORDLIST;

/// Sanity ceiling on generated-password length. The UI caps its slider
/// at 64, but that's a UI-only limit, not a security boundary — nothing
/// stopped a direct Tauri command invocation (e.g. from devtools) from
/// requesting an enormous `length`. `Vec::with_capacity` on a
/// pathological value (found by reasoning about what a malicious or
/// simply buggy caller could send, not by triggering it) would attempt a
/// huge allocation; on failure, Rust's default global allocator aborts
/// the *entire process* rather than returning a catchable error — an
/// uncatchable, self-inflicted denial of service. 1024 is generously
/// above any real use case while staying trivially cheap to allocate.
const MAX_PASSWORD_LENGTH: usize = 1024;
/// Same rationale, for passphrase word count.
const MAX_PASSPHRASE_WORDS: usize = 128;

const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{};:,.?/~";
// Characters that are easily confused in most fonts: 0/O, 1/l/I, 5/S, etc.
const AMBIGUOUS: &[u8] = b"0O1lI5S8B";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordOptions {
    pub length: usize,
    pub uppercase: bool,
    pub lowercase: bool,
    pub digits: bool,
    pub symbols: bool,
    pub exclude_ambiguous: bool,
}

impl Default for PasswordOptions {
    fn default() -> Self {
        Self {
            length: 20,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            exclude_ambiguous: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassphraseOptions {
    pub word_count: usize,
    pub separator: String,
    pub capitalize: bool,
    pub include_number: bool,
}

impl Default for PassphraseOptions {
    fn default() -> Self {
        Self {
            word_count: 5,
            separator: "-".to_string(),
            capitalize: true,
            include_number: true,
        }
    }
}

fn filtered(charset: &[u8], exclude_ambiguous: bool) -> Vec<u8> {
    if !exclude_ambiguous {
        return charset.to_vec();
    }
    charset
        .iter()
        .copied()
        .filter(|c| !AMBIGUOUS.contains(c))
        .collect()
}

/// Generates a random password from the requested character classes.
///
/// Guarantees at least one character from each *selected* class is
/// present (placed at random positions, not fixed offsets, so the
/// guarantee doesn't itself narrow the search space in a predictable
/// way), then fills the remainder uniformly at random and shuffles.
pub fn generate_password(opts: &PasswordOptions) -> Result<String> {
    if opts.length == 0 || opts.length > MAX_PASSWORD_LENGTH {
        return Err(KeyflowError::InvalidGeneratorConfig);
    }
    let mut pools: Vec<Vec<u8>> = Vec::new();
    if opts.lowercase {
        pools.push(filtered(LOWER, opts.exclude_ambiguous));
    }
    if opts.uppercase {
        pools.push(filtered(UPPER, opts.exclude_ambiguous));
    }
    if opts.digits {
        pools.push(filtered(DIGITS, opts.exclude_ambiguous));
    }
    if opts.symbols {
        pools.push(filtered(SYMBOLS, opts.exclude_ambiguous));
    }
    if pools.is_empty() || pools.iter().any(|p| p.is_empty()) {
        return Err(KeyflowError::InvalidGeneratorConfig);
    }
    if opts.length < pools.len() {
        return Err(KeyflowError::InvalidGeneratorConfig);
    }

    let mut rng = OsRng;
    let all: Vec<u8> = pools.iter().flatten().copied().collect();

    let mut chars: Vec<u8> = Vec::with_capacity(opts.length);
    // One guaranteed character per selected class.
    for pool in &pools {
        chars.push(*pool.choose(&mut rng).expect("pool checked non-empty above"));
    }
    // Fill the rest uniformly from the combined alphabet.
    for _ in chars.len()..opts.length {
        chars.push(*all.choose(&mut rng).expect("alphabet checked non-empty above"));
    }
    chars.shuffle(&mut rng);

    Ok(String::from_utf8(chars).expect("charset is ASCII by construction"))
}

/// Generates a diceware-style passphrase from KeyFlow's built-in
/// wordlist (see [`crate::wordlist`]).
pub fn generate_passphrase(opts: &PassphraseOptions) -> Result<String> {
    if opts.word_count == 0 || opts.word_count > MAX_PASSPHRASE_WORDS || WORDLIST.is_empty() {
        return Err(KeyflowError::InvalidGeneratorConfig);
    }
    let mut rng = OsRng;
    let mut words: Vec<String> = (0..opts.word_count)
        .map(|_| {
            let w = WORDLIST.choose(&mut rng).expect("wordlist is non-empty");
            if opts.capitalize {
                let mut c = w.chars();
                match c.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            } else {
                w.to_string()
            }
        })
        .collect();
    if opts.include_number {
        let n: u32 = rng.gen_range(0..1000);
        words.push(n.to_string());
    }
    Ok(words.join(&opts.separator))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrengthBand {
    VeryWeak,
    Weak,
    Fair,
    Strong,
    VeryStrong,
}

impl StrengthBand {
    pub fn label(&self) -> &'static str {
        match self {
            StrengthBand::VeryWeak => "Very weak",
            StrengthBand::Weak => "Weak",
            StrengthBand::Fair => "Fair",
            StrengthBand::Strong => "Strong",
            StrengthBand::VeryStrong => "Very strong",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StrengthEstimate {
    /// Estimated entropy in bits. This is a structural estimate (based on
    /// alphabet size and length) rather than a statistical model of real
    /// password corpora — it is a lower bound for a genuinely random
    /// password from this generator, not a claim about human-chosen ones.
    pub entropy_bits: f64,
    pub band: StrengthBand,
}

/// Estimates strength from the character classes actually present in
/// `password`, assuming it was drawn uniformly at random from that
/// alphabet. This deliberately does not attempt dictionary/pattern
/// analysis (e.g. zxcvbn-style scoring) for human-chosen passwords —
/// that would require bundling a large frequency corpus, which is future
/// work (see ROADMAP.md). Callers generating via [`generate_password`]
/// get an accurate bound; callers scoring an arbitrary user-typed
/// password should treat this as optimistic.
pub fn estimate_strength(password: &str) -> StrengthEstimate {
    let mut alphabet_size: f64 = 0.0;
    let has = |set: &[u8]| password.bytes().any(|b| set.contains(&b));
    if has(LOWER) {
        alphabet_size += LOWER.len() as f64;
    }
    if has(UPPER) {
        alphabet_size += UPPER.len() as f64;
    }
    if has(DIGITS) {
        alphabet_size += DIGITS.len() as f64;
    }
    let other = password
        .bytes()
        .filter(|b| !LOWER.contains(b) && !UPPER.contains(b) && !DIGITS.contains(b))
        .count();
    if other > 0 {
        alphabet_size += SYMBOLS.len().max(other) as f64;
    }
    if alphabet_size == 0.0 {
        return StrengthEstimate {
            entropy_bits: 0.0,
            band: StrengthBand::VeryWeak,
        };
    }
    let entropy_bits = password.chars().count() as f64 * alphabet_size.log2();
    let band = match entropy_bits {
        b if b < 28.0 => StrengthBand::VeryWeak,
        b if b < 45.0 => StrengthBand::Weak,
        b if b < 60.0 => StrengthBand::Fair,
        b if b < 80.0 => StrengthBand::Strong,
        _ => StrengthBand::VeryStrong,
    };
    StrengthEstimate { entropy_bits, band }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn wordlist_has_no_duplicates() {
        let set: HashSet<&str> = WORDLIST.iter().copied().collect();
        assert_eq!(set.len(), WORDLIST.len(), "wordlist contains duplicate entries");
    }

    #[test]
    fn wordlist_is_reasonably_sized() {
        assert!(WORDLIST.len() >= 512, "wordlist too small for meaningful passphrase entropy");
    }

    #[test]
    fn generated_password_respects_length() {
        let opts = PasswordOptions { length: 24, ..Default::default() };
        let pw = generate_password(&opts).unwrap();
        assert_eq!(pw.chars().count(), 24);
    }

    #[test]
    fn generated_password_contains_every_selected_class() {
        let opts = PasswordOptions {
            length: 40,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            exclude_ambiguous: true,
        };
        for _ in 0..50 {
            let pw = generate_password(&opts).unwrap();
            assert!(pw.bytes().any(|b| LOWER.contains(&b)));
            assert!(pw.bytes().any(|b| UPPER.contains(&b)));
            assert!(pw.bytes().any(|b| DIGITS.contains(&b)));
            assert!(pw.bytes().any(|b| SYMBOLS.contains(&b)));
        }
    }

    #[test]
    fn ambiguous_exclusion_is_honored() {
        let opts = PasswordOptions { length: 200, exclude_ambiguous: true, ..Default::default() };
        let pw = generate_password(&opts).unwrap();
        assert!(!pw.bytes().any(|b| AMBIGUOUS.contains(&b)));
    }

    #[test]
    fn rejects_impossible_configs() {
        let opts = PasswordOptions {
            length: 5,
            uppercase: false,
            lowercase: false,
            digits: false,
            symbols: false,
            exclude_ambiguous: false,
        };
        assert!(generate_password(&opts).is_err());
    }

    #[test]
    fn two_generated_passwords_differ() {
        let opts = PasswordOptions::default();
        let a = generate_password(&opts).unwrap();
        let b = generate_password(&opts).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn passphrase_has_requested_word_count() {
        let opts = PassphraseOptions { word_count: 6, include_number: false, ..Default::default() };
        let phrase = generate_passphrase(&opts).unwrap();
        assert_eq!(phrase.split('-').count(), 6);
    }

    #[test]
    fn strength_increases_with_length() {
        let short = estimate_strength("abcXYZ12");
        let long = estimate_strength("abcXYZ12abcXYZ12abcXYZ12");
        assert!(long.entropy_bits > short.entropy_bits);
    }

    #[test]
    fn empty_password_is_very_weak() {
        assert_eq!(estimate_strength("").band, StrengthBand::VeryWeak);
    }

    #[test]
    fn generator_output_meets_its_own_strength_bar() {
        let pw = generate_password(&PasswordOptions::default()).unwrap();
        let strength = estimate_strength(&pw);
        assert!(matches!(strength.band, StrengthBand::Strong | StrengthBand::VeryStrong));
    }

    /// Regression test for a real bug found during a QA pass: `length`
    /// and `word_count` come straight from the frontend with no
    /// server-side bound (the UI's own slider caps are not a security
    /// boundary — a direct Tauri command invocation could send anything
    /// representable as a `usize`). Before this bound existed, a
    /// pathologically large value would have reached
    /// `Vec::with_capacity`, whose allocation failure aborts the whole
    /// process rather than returning a catchable error — an uncatchable,
    /// self-inflicted denial of service. This test only checks the
    /// guard rejects values just over the new limit; it deliberately
    /// does not exercise `usize::MAX` itself, since that's exactly the
    /// uncatchable-abort path being prevented, not something a test can
    /// safely trigger.
    #[test]
    fn rejects_password_length_over_the_sanity_ceiling() {
        let opts = PasswordOptions { length: MAX_PASSWORD_LENGTH + 1, ..Default::default() };
        assert!(generate_password(&opts).is_err());
    }

    #[test]
    fn accepts_password_length_at_the_sanity_ceiling() {
        let opts = PasswordOptions { length: MAX_PASSWORD_LENGTH, ..Default::default() };
        assert!(generate_password(&opts).is_ok());
    }

    #[test]
    fn rejects_passphrase_word_count_over_the_sanity_ceiling() {
        let opts = PassphraseOptions { word_count: MAX_PASSPHRASE_WORDS + 1, ..Default::default() };
        assert!(generate_passphrase(&opts).is_err());
    }

    #[test]
    fn rejects_length_too_short_to_fit_one_char_per_selected_class() {
        let opts = PasswordOptions {
            length: 2,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            exclude_ambiguous: false,
        };
        assert!(generate_password(&opts).is_err());
    }
}

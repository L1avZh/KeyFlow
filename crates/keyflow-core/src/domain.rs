//! Origin and domain matching for credential autofill.
//!
//! This is the code that decides whether a saved credential may be
//! offered on the page the user is currently looking at. Getting it
//! wrong in the permissive direction is a phishing vector, so every rule
//! here is deliberately conservative: on doubt, don't match.
//!
//! # What this does implement
//! - Same-origin exact matching (scheme + host + port).
//! - Parent-domain-to-subdomain matching (a credential saved for
//!   `example.com` is offered on `login.example.com`, never the reverse).
//! - IDNA/punycode normalization so `xn--` and Unicode forms of the same
//!   host compare equal.
//! - HTTP→HTTPS upgrade tolerance, and a hard block on the reverse
//!   (HTTPS→HTTP is treated as a possible downgrade attack, not matched).
//! - Special-cased `localhost`/`.localhost`/`.test` handling so local
//!   development ports don't break matching.
//! - IP-literal hosts are matched exactly (never treated as having
//!   "subdomains").
//!
//! # Known limitation
//! Real eTLD+1 computation requires the Mozilla Public Suffix List
//! (thousands of entries, updated regularly). Bundling and refreshing
//! that list is tracked in ROADMAP.md. Until then, this module ships a
//! small built-in list of common multi-label public suffixes
//! (`co.uk`, `com.au`, `github.io`, ...) and otherwise assumes the last
//! label is the suffix. This is correct for the overwhelming majority of
//! real domains but is not authoritative — see THREAT_MODEL.md.
//!
//! This module also does not defend against homograph/confusable-script
//! attacks (e.g. a Cyrillic lookalike of a Latin domain); it normalizes
//! to punycode for comparison but does not flag visually-confusable
//! domains to the user. That is a UI-layer responsibility tracked in
//! THREAT_MODEL.md.

use std::net::IpAddr;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{KeyflowError, Result};

/// Small built-in set of common multi-label public suffixes. Not
/// authoritative — see module docs.
const KNOWN_MULTI_LABEL_SUFFIXES: &[&str] = &[
    "co.uk", "org.uk", "gov.uk", "ac.uk", "me.uk", "ltd.uk", "plc.uk",
    "com.au", "net.au", "org.au", "edu.au", "gov.au",
    "co.nz", "net.nz", "org.nz",
    "co.jp", "or.jp", "ne.jp",
    "com.br", "net.br",
    "com.cn", "net.cn", "org.cn",
    "co.in", "net.in", "org.in", "gen.in",
    "co.za", "org.za",
    "co.il", "org.il",
    "com.mx", "net.mx",
    "co.kr",
    "com.sg", "net.sg",
    "com.tw",
    "github.io", "gitlab.io", "pages.dev", "netlify.app", "vercel.app", "web.app",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    pub scheme: String,
    pub host: String,
    pub port: Option<u16>,
}

impl Origin {
    /// Parses and normalizes a URL into an [`Origin`]. Hosts are
    /// lowercased and IDNA-normalized to ASCII (punycode) so Unicode and
    /// `xn--` forms of the same host compare equal.
    pub fn parse(url_str: &str) -> Result<Self> {
        let url = Url::parse(url_str).map_err(|_| KeyflowError::InvalidDomain(url_str.to_string()))?;
        let host = url
            .host_str()
            .ok_or_else(|| KeyflowError::InvalidDomain(url_str.to_string()))?;
        let normalized_host = normalize_host(host)?;
        Ok(Self {
            scheme: url.scheme().to_ascii_lowercase(),
            host: normalized_host,
            port: url.port(),
        })
    }

    /// True if this origin's host is a literal IP address — IPv4
    /// (`192.168.1.1`) or IPv6, which `url::Url::host_str()` returns in
    /// *bracketed* form (`[::1]`, per the WHATWG URL spec), unlike
    /// `std::net::IpAddr`'s parser, which rejects brackets. Found via a
    /// real parse-and-print probe, not by inspection alone: without
    /// stripping the brackets first, every IPv6 host silently fell
    /// through `is_ip()` as `false` and was instead run through the
    /// dot-label domain-matching logic below — harmless for a bare IPv6
    /// address (no dots to split on), but for an IPv4-mapped IPv6
    /// literal like `[::ffff:192.168.1.1]` (which *does* contain dots),
    /// that logic would treat it as a hierarchical DNS name with a fake
    /// "TLD", defeating the "IP host never treated as having subdomains"
    /// guarantee this module otherwise enforces.
    fn is_ip(&self) -> bool {
        let unbracketed = self.host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(&self.host);
        IpAddr::from_str(unbracketed).is_ok()
    }

    fn is_dev_host(&self) -> bool {
        self.host == "localhost" || self.host.ends_with(".localhost") || self.host.ends_with(".test")
    }

    fn effective_port(&self) -> u16 {
        self.port.unwrap_or(match self.scheme.as_str() {
            "https" => 443,
            "http" => 80,
            _ => 0,
        })
    }
}

fn normalize_host(host: &str) -> Result<String> {
    let ascii = idna::domain_to_ascii(host).map_err(|_| KeyflowError::InvalidDomain(host.to_string()))?;
    Ok(ascii.trim_end_matches('.').to_ascii_lowercase())
}

/// Computes a simplified "registrable domain" (eTLD+1) for a host. See
/// module-level docs for the accuracy caveat.
fn registrable_domain(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() <= 1 {
        return host.to_string();
    }
    let last_two = format!("{}.{}", labels[labels.len() - 2], labels[labels.len() - 1]);
    if KNOWN_MULTI_LABEL_SUFFIXES.contains(&last_two.as_str()) && labels.len() >= 3 {
        format!(
            "{}.{}",
            labels[labels.len() - 3],
            last_two
        )
    } else {
        last_two
    }
}

/// Returns true if `host` is (as far as this simplified suffix list
/// knows) itself nothing but a public suffix — a single label ("com"),
/// or a known two-label suffix ("co.uk") — with no registrable owner
/// label in front of it. Such a host must never be treated as a saved
/// credential's "domain" for subdomain-wildcard purposes.
fn is_public_suffix(host: &str) -> bool {
    match host.split('.').count() {
        0 | 1 => true,
        2 => KNOWN_MULTI_LABEL_SUFFIXES.contains(&host),
        _ => false,
    }
}

/// Returns true if `candidate` is `saved` itself or a subdomain of it,
/// checked on label boundaries (never a naive string-suffix check, which
/// would wrongly match `evil-example.com` against `example.com`).
fn is_equal_or_subdomain(saved: &str, candidate: &str) -> bool {
    if saved == candidate {
        return true;
    }
    candidate
        .strip_suffix(saved)
        .map(|prefix| prefix.ends_with('.'))
        .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchDecision {
    /// Identical scheme, host, and (effective) port.
    ExactMatch,
    /// Same registrable domain, candidate is `saved` or a subdomain of
    /// it, optionally with an HTTP→HTTPS upgrade.
    SubdomainMatch,
    /// Looked plausible but was rejected for a specific, nameable
    /// security reason (e.g. a scheme downgrade). Never autofill on this.
    Blocked { reason: String },
    /// No relationship between the two origins.
    NoMatch,
}

impl MatchDecision {
    pub fn should_offer_autofill(&self) -> bool {
        matches!(self, MatchDecision::ExactMatch | MatchDecision::SubdomainMatch)
    }
}

/// Decides whether `candidate` (the page currently being viewed) may be
/// offered the credential saved for `saved` (the origin recorded at save
/// time).
pub fn evaluate_match(saved: &Origin, candidate: &Origin) -> MatchDecision {
    if saved.is_ip() || candidate.is_ip() {
        return if saved.host == candidate.host
            && saved.scheme == candidate.scheme
            && saved.effective_port() == candidate.effective_port()
        {
            MatchDecision::ExactMatch
        } else {
            MatchDecision::NoMatch
        };
    }

    let same_host = saved.host == candidate.host;
    if !same_host {
        if !is_equal_or_subdomain(&saved.host, &candidate.host) {
            return MatchDecision::NoMatch;
        }
        // Guard against a credential saved directly against a bare
        // public suffix (e.g. "co.uk", or a single-label host) ever
        // acting as a wildcard for everything under it.
        if is_public_suffix(&saved.host) {
            return MatchDecision::NoMatch;
        }
        if registrable_domain(&saved.host) != registrable_domain(&candidate.host) {
            return MatchDecision::NoMatch;
        }
    }

    // From here, the host relationship is established: either identical
    // hosts, or `candidate` is a genuine subdomain of `saved` within the
    // same registrable domain. Scheme/port are evaluated the same way
    // for both cases.
    if saved.scheme == "https" && candidate.scheme == "http" {
        return MatchDecision::Blocked {
            reason: "saved login uses HTTPS but this page is plain HTTP (possible downgrade)".into(),
        };
    }

    let dev = saved.is_dev_host() || candidate.is_dev_host();
    let ports_ok = if dev {
        true
    } else if saved.scheme == candidate.scheme {
        saved.effective_port() == candidate.effective_port()
    } else {
        // Scheme upgrade (http -> https): only compatible if neither
        // side pins an explicit, differing port. Comparing default
        // ports (80 vs 443) here would always spuriously fail.
        saved.port == candidate.port
    };
    if !ports_ok {
        return MatchDecision::NoMatch;
    }

    if same_host && saved.scheme == candidate.scheme {
        MatchDecision::ExactMatch
    } else {
        MatchDecision::SubdomainMatch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(s: &str) -> Origin {
        Origin::parse(s).unwrap()
    }

    #[test]
    fn exact_same_origin_matches() {
        let saved = origin("https://example.com/login");
        let candidate = origin("https://example.com/account");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::ExactMatch);
    }

    #[test]
    fn subdomain_of_saved_domain_matches() {
        let saved = origin("https://example.com");
        let candidate = origin("https://login.example.com");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::SubdomainMatch);
    }

    #[test]
    fn reverse_subdomain_direction_does_not_match() {
        let saved = origin("https://login.example.com");
        let candidate = origin("https://example.com");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::NoMatch);
    }

    #[test]
    fn suffix_appended_attacker_domain_does_not_match() {
        let saved = origin("https://example.com");
        let candidate = origin("https://example.com.attacker.com");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::NoMatch);
    }

    #[test]
    fn hyphenated_lookalike_domain_does_not_match() {
        let saved = origin("https://example.com");
        let candidate = origin("https://attacker-example.com");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::NoMatch);
    }

    #[test]
    fn unrelated_domain_does_not_match() {
        let saved = origin("https://example.com");
        let candidate = origin("https://totally-different.com");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::NoMatch);
    }

    #[test]
    fn http_to_https_upgrade_on_same_host_matches() {
        let saved = origin("http://example.com");
        let candidate = origin("https://example.com");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::SubdomainMatch);
    }

    #[test]
    fn https_to_http_downgrade_is_blocked_not_matched() {
        let saved = origin("https://example.com");
        let candidate = origin("http://example.com");
        let result = evaluate_match(&saved, &candidate);
        assert!(!result.should_offer_autofill());
        assert!(matches!(result, MatchDecision::Blocked { .. }));
    }

    #[test]
    fn different_ports_on_public_host_do_not_match() {
        let saved = origin("https://example.com:8443");
        let candidate = origin("https://example.com:9443");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::NoMatch);
    }

    #[test]
    fn localhost_ignores_port_differences() {
        let saved = origin("http://localhost:3000");
        let candidate = origin("http://localhost:8080");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::ExactMatch);
    }

    #[test]
    fn ip_addresses_require_exact_match() {
        let saved = origin("http://192.168.1.10:8080");
        let candidate = origin("http://192.168.1.10:8080");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::ExactMatch);
    }

    #[test]
    fn different_ip_addresses_do_not_match() {
        let saved = origin("http://192.168.1.10");
        let candidate = origin("http://192.168.1.11");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::NoMatch);
    }

    #[test]
    fn ip_host_never_treated_as_having_subdomains() {
        // Would be nonsensical, but make sure an IP is never routed
        // through the label-suffix subdomain logic.
        let saved = origin("http://1.1.1.1");
        let candidate = origin("http://evil.1.1.1.1.attacker.com");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::NoMatch);
    }

    /// Regression test for a real bug found during a QA pass:
    /// `url::Url::host_str()` returns IPv6 hosts bracketed (`"[::1]"`,
    /// per the WHATWG URL spec), but `std::net::IpAddr::from_str`
    /// rejects brackets — so `is_ip()` returned `false` for every IPv6
    /// host before the fix, routing it through the dot-label
    /// domain-matching logic instead of the dedicated IP-exact-match
    /// path.
    #[test]
    fn ipv6_loopback_matches_itself_exactly() {
        let saved = origin("http://[::1]:8080");
        let candidate = origin("http://[::1]:8080");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::ExactMatch);
    }

    #[test]
    fn different_ipv6_addresses_do_not_match() {
        let saved = origin("http://[::1]");
        let candidate = origin("http://[::2]");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::NoMatch);
    }

    /// The specific failure mode the bug allowed: an IPv4-mapped IPv6
    /// literal contains embedded dots (unlike a bare IPv6 address), so
    /// if it were ever routed through the domain-matching path instead
    /// of the IP-exact-match path, `host.split('.')` would carve it into
    /// fake DNS-style labels — here, two *different* mapped addresses
    /// that happen to share a "1]" trailing label must never be treated
    /// as a domain/subdomain pair.
    #[test]
    fn ipv4_mapped_ipv6_literal_is_never_treated_as_a_hierarchical_domain() {
        let saved = origin("http://[::ffff:192.168.1.1]");
        let candidate = origin("http://[::ffff:10.0.0.1]");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::NoMatch);
    }

    #[test]
    fn ipv4_mapped_ipv6_literal_matches_itself_exactly() {
        let saved = origin("http://[::ffff:192.168.1.1]:9000");
        let candidate = origin("http://[::ffff:192.168.1.1]:9000");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::ExactMatch);
    }

    #[test]
    fn known_two_label_public_suffix_is_respected() {
        // "example.co.uk" is the registrable domain; "co.uk" alone must
        // never act as a wildcard suffix for unrelated sites.
        let saved = origin("https://example.co.uk");
        let candidate = origin("https://portal.example.co.uk");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::SubdomainMatch);

        let unrelated_saved = origin("https://co.uk");
        let unrelated_candidate = origin("https://anything-else.co.uk");
        assert_eq!(evaluate_match(&unrelated_saved, &unrelated_candidate), MatchDecision::NoMatch);
    }

    #[test]
    fn punycode_and_unicode_forms_of_same_host_are_equal() {
        let unicode = origin("https://münchen.example");
        let punycode = origin("https://xn--mnchen-3ya.example");
        assert_eq!(unicode.host, punycode.host);
    }

    #[test]
    fn dev_test_tld_is_treated_like_localhost() {
        let saved = origin("http://myapp.test:3000");
        let candidate = origin("http://myapp.test:4000");
        assert_eq!(evaluate_match(&saved, &candidate), MatchDecision::ExactMatch);
    }
}

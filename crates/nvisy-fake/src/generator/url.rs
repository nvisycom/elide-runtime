//! URL synthesis with hostname-safe username sanitisation.

use fake::faker::internet::raw as internet;
use fake::rand::RngExt;

use super::dispatch::dispatch;
use crate::locale::Locale;

pub(super) fn url<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let user: String = dispatch!(locale, rng, internet::Username);
    let domain: String = dispatch!(locale, rng, internet::DomainSuffix);
    let host = sanitise_hostname_label(&user);
    let host = if host.is_empty() {
        "site"
    } else {
        host.as_str()
    };
    format!("https://www.{host}.{domain}")
}

/// Strip characters that aren't valid in a DNS label
/// (RFC 1035: ASCII letters, digits, and hyphens), and trim leading
/// or trailing hyphens. Returns lowercase output.
fn sanitise_hostname_label(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            out.push(ch.to_ascii_lowercase());
        }
    }
    let trimmed = out.trim_matches('-');
    trimmed.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_non_dns_characters() {
        assert_eq!(sanitise_hostname_label("Ali_ce"), "alice");
        assert_eq!(sanitise_hostname_label("Bob.Smith"), "bobsmith");
        assert_eq!(sanitise_hostname_label("-mid-"), "mid");
    }

    #[test]
    fn handles_empty_after_strip() {
        assert!(sanitise_hostname_label("___").is_empty());
    }
}

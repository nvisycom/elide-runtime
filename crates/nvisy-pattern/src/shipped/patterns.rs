//! Built-in [`Regex`] rules, embedded at compile time.
//!
//! Each accessor returns a fresh [`Regex`] parsed from a JSON
//! definition file under `assets/patterns/`. The parse happens on
//! every call — rules are cheap to construct since
//! [`PatternRecognizer::build`] does the heavy compilation.
//!
//! [`Regex`]: crate::Regex
//! [`PatternRecognizer::build`]: crate::PatternRecognizer

use crate::Regex;

macro_rules! shipped_pattern {
    ($(#[$meta:meta])* fn $name:ident from $path:literal) => {
        $(#[$meta])*
        #[must_use]
        pub fn $name() -> Regex {
            Regex::from_json(include_bytes!(concat!("../../assets/patterns/", $path)))
                .expect(concat!("shipped pattern `", $path, "` is well-formed"))
        }
    };
}

shipped_pattern!(
    /// Email address (RFC-loose).
    fn email from "contact/email.json"
);
shipped_pattern!(
    /// International phone numbers.
    fn phone from "contact/phone.json"
);
shipped_pattern!(
    /// URLs (HTTP/HTTPS/FTP).
    fn url from "contact/url.json"
);

shipped_pattern!(
    /// AWS access key IDs.
    fn aws_key from "credentials/aws_key.json"
);
shipped_pattern!(
    /// Heuristic generic API key.
    fn generic_api_key from "credentials/generic_api_key.json"
);
shipped_pattern!(
    /// GitHub personal access tokens.
    fn github_token from "credentials/github_token.json"
);
shipped_pattern!(
    /// PEM-formatted private keys.
    fn private_key from "credentials/private_key.json"
);
shipped_pattern!(
    /// Stripe live/test secret keys.
    fn stripe_key from "credentials/stripe_key.json"
);

shipped_pattern!(
    /// Bitcoin (legacy + bech32) addresses.
    fn bitcoin_address from "finance/bitcoin_address.json"
);
shipped_pattern!(
    /// Credit-card numbers, Luhn-validated.
    fn credit_card from "finance/credit_card.json"
);
shipped_pattern!(
    /// Ethereum addresses.
    fn ethereum_address from "finance/ethereum_address.json"
);
shipped_pattern!(
    /// International Bank Account Numbers.
    fn iban from "finance/iban.json"
);
shipped_pattern!(
    /// SWIFT / BIC codes.
    fn swift_code from "finance/swift_code.json"
);
shipped_pattern!(
    /// US bank routing numbers (ABA RTN).
    fn us_bank_routing from "finance/us_bank_routing.json"
);

shipped_pattern!(
    /// US Social Security numbers (AAA-GG-SSSS).
    fn ssn from "identity/ssn.json"
);
shipped_pattern!(
    /// US driver's license numbers.
    fn us_drivers_license from "identity/us_drivers_license.json"
);
shipped_pattern!(
    /// US passport numbers.
    fn us_passport from "identity/us_passport.json"
);
shipped_pattern!(
    /// US ZIP and ZIP+4 postal codes.
    fn us_postal_code from "identity/us_postal_code.json"
);

shipped_pattern!(
    /// IPv4 addresses.
    fn ipv4 from "network/ipv4.json"
);
shipped_pattern!(
    /// IPv6 addresses.
    fn ipv6 from "network/ipv6.json"
);
shipped_pattern!(
    /// MAC (Ethernet) addresses.
    fn mac_address from "network/mac_address.json"
);

shipped_pattern!(
    /// Date of birth in common written formats.
    fn date_of_birth from "personal/date_of_birth.json"
);
shipped_pattern!(
    /// Date + time stamps in ISO-like formats.
    fn datetime from "personal/datetime.json"
);

/// Every built-in regex pattern shipped by this crate, in arbitrary
/// stable order.
#[must_use]
pub fn all() -> Vec<Regex> {
    vec![
        email(),
        phone(),
        url(),
        aws_key(),
        generic_api_key(),
        github_token(),
        private_key(),
        stripe_key(),
        bitcoin_address(),
        credit_card(),
        ethereum_address(),
        iban(),
        swift_code(),
        us_bank_routing(),
        ssn(),
        us_drivers_license(),
        us_passport(),
        us_postal_code(),
        ipv4(),
        ipv6(),
        mac_address(),
        date_of_birth(),
        datetime(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_pattern_parses() {
        let patterns = all();
        assert_eq!(patterns.len(), 23);
    }
}

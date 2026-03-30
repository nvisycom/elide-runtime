//! Broad entity category classification.
//!
//! [`EntityCategory`] groups related [`EntityKind`](super::EntityKind)
//! variants into policy-addressable buckets.  Policy selectors can
//! target an entire category (e.g. "redact all financial data") without
//! enumerating individual kinds.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Broad category of sensitive data.
///
/// Each [`EntityKind`](super::EntityKind) maps to exactly one category
/// via [`EntityKind::category()`](super::EntityKind::category).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum EntityCategory {
    /// Personal identity: names, government IDs, dates of birth, and
    /// other attributes that directly identify a natural person.
    PersonalIdentity,
    /// Contact information: email addresses, phone numbers, physical
    /// addresses, postal codes, and URLs.
    ContactInfo,
    /// Demographic attributes: age, gender, ethnicity, religion,
    /// nationality, and citizenship.
    Demographic,
    /// Financial instruments and accounts: payment cards, bank
    /// accounts, routing numbers, IBAN, crypto addresses, and
    /// monetary amounts.
    Financial,
    /// Protected health information: medical record numbers,
    /// insurance IDs, prescriptions, diagnoses, and medications.
    Health,
    /// Biometric identifiers: fingerprints, voiceprints, retina
    /// scans, and facial geometry templates.
    Biometric,
    /// Secrets and credentials: passwords, API keys, authentication
    /// tokens, and private cryptographic keys.
    Credentials,
    /// Network and device identifiers: IP addresses, MAC addresses,
    /// device IDs, and usernames.
    NetworkIdentifier,
    /// Geographic and spatial data: GPS coordinates and geolocation
    /// metadata.
    Location,
    /// Sensitive visual elements detected in images or video:
    /// faces, handwriting, signatures, logos, and barcodes.
    Visual,
    /// Organizational identifiers: company names, departments,
    /// facilities, and institutional reference numbers.
    Organizational,
}

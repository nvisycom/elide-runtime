//! Concrete entity kind enumeration.
//!
//! [`EntityKind`] enumerates the types of sensitive data the platform
//! can detect or redact.  Each variant maps to a stable `snake_case`
//! string for serialization and display.
//!
//! Every variant also maps to an [`EntityCategory`] via [`EntityKind::category`]
//! and an [`EntitySensitivity`] via [`EntityKind::sensitivity`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::category::EntityCategory;
use super::sensitivity::EntitySensitivity;

/// Specific kind of sensitive entity detected or targeted for redaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EntityKind {
    // ── Identity documents:
    /// Government-issued identification number (SSN, SIN, Aadhaar, national ID, etc.).
    GovernmentId,
    /// Tax identification number (ITIN, EIN, TIN, etc.).
    TaxId,
    /// Driver's license number.
    DriversLicense,
    /// Passport number.
    PassportNumber,
    /// Vehicle identification number (VIN).
    VehicleId,
    /// License plate number.
    LicensePlate,

    // ── Personal information:
    /// Person name (full, first, or last).
    PersonName,
    /// Date of birth.
    DateOfBirth,
    /// Age value.
    Age,
    /// Demographic attribute (gender, race/ethnicity, religion, orientation, etc.).
    Demographic,

    // ── Contact information:
    /// Email address.
    EmailAddress,
    /// Phone number.
    PhoneNumber,
    /// Physical / mailing address.
    Address,
    /// Postal or ZIP code.
    PostalCode,
    /// URL or hyperlink.
    Url,

    // ── Network & device identifiers:
    /// IP address (v4 or v6).
    IpAddress,
    /// MAC (hardware) address.
    MacAddress,
    /// Device identifier (IMEI, IDFA, etc.).
    DeviceId,
    /// Username or online handle.
    Username,

    // ── Financial:
    /// Payment card number (credit or debit).
    PaymentCard,
    /// Payment card security code (CVV/CVC).
    CardSecurityCode,
    /// Payment card expiration date.
    CardExpiry,
    /// Bank account number.
    BankAccount,
    /// Bank routing / transit number.
    BankRouting,
    /// International Bank Account Number (IBAN).
    Iban,
    /// SWIFT / BIC code.
    SwiftCode,
    /// Monetary amount.
    Amount,
    /// Cryptocurrency wallet address.
    CryptoAddress,

    // ── Health:
    /// Medical or patient identifier.
    MedicalId,
    /// Insurance policy number.
    InsuranceId,
    /// Prescription number.
    PrescriptionId,

    // ── Credentials:
    /// Password or passphrase.
    Password,
    /// API key.
    ApiKey,
    /// Authentication or session token.
    AuthToken,
    /// Private cryptographic key.
    PrivateKey,

    // ── Biometric:
    /// Fingerprint template or minutiae data.
    Fingerprint,
    /// Voiceprint / speaker embedding.
    Voiceprint,
    /// Retina or iris scan data.
    RetinaScan,
    /// Facial geometry / face embedding (not a photo — see [`Face`](Self::Face)).
    FacialGeometry,

    // ── Location:
    /// GPS coordinates (latitude / longitude).
    Coordinates,
    /// Geolocation metadata (EXIF, cell tower, etc.).
    GeolocationMetadata,

    // ── Dates & times:
    /// Date and/or time value.
    DateTime,

    // ── Organizations:
    /// Company or organisation name.
    OrganizationName,

    // ── Visual / image entities:
    /// Detected human face in an image.
    Face,
    /// Handwritten text region.
    Handwriting,
    /// Handwritten or digital signature.
    Signature,
    /// Logo or brand mark.
    Logo,
    /// Barcode (1D) or QR code (2D).
    Barcode,
}

impl EntityKind {
    /// Returns the [`EntityCategory`] this entity kind belongs to.
    pub fn category(&self) -> EntityCategory {
        match self {
            // Identity & personal
            Self::GovernmentId
            | Self::TaxId
            | Self::DriversLicense
            | Self::PassportNumber
            | Self::VehicleId
            | Self::LicensePlate
            | Self::PersonName
            | Self::DateOfBirth
            | Self::Age
            | Self::Demographic => EntityCategory::Pii,

            // Contact
            Self::EmailAddress
            | Self::PhoneNumber
            | Self::Address
            | Self::PostalCode
            | Self::Url => EntityCategory::Pii,

            // Network & device
            Self::IpAddress
            | Self::MacAddress
            | Self::DeviceId
            | Self::Username => EntityCategory::Pii,

            // Financial
            Self::PaymentCard
            | Self::CardSecurityCode
            | Self::CardExpiry
            | Self::BankAccount
            | Self::BankRouting
            | Self::Iban
            | Self::SwiftCode
            | Self::Amount
            | Self::CryptoAddress => EntityCategory::Financial,

            // Health
            Self::MedicalId
            | Self::InsuranceId
            | Self::PrescriptionId => EntityCategory::Phi,

            // Credentials
            Self::Password
            | Self::ApiKey
            | Self::AuthToken
            | Self::PrivateKey => EntityCategory::Credentials,

            // Biometric
            Self::Fingerprint
            | Self::Voiceprint
            | Self::RetinaScan
            | Self::FacialGeometry
            | Self::Face => EntityCategory::Biometric,

            // Location
            Self::Coordinates | Self::GeolocationMetadata => EntityCategory::Pii,

            // Dates & times
            Self::DateTime => EntityCategory::Pii,

            // Organizations
            Self::OrganizationName => EntityCategory::Pii,

            // Visual / image
            Self::Handwriting
            | Self::Signature
            | Self::Logo
            | Self::Barcode => EntityCategory::Pii,
        }
    }

    /// Returns the default [`EntitySensitivity`] for this entity kind.
    pub fn sensitivity(&self) -> EntitySensitivity {
        match self {
            // Critical: irrevocable identifiers, secrets, biometrics
            Self::GovernmentId
            | Self::PassportNumber
            | Self::PaymentCard
            | Self::CardSecurityCode
            | Self::BankAccount
            | Self::Password
            | Self::ApiKey
            | Self::AuthToken
            | Self::PrivateKey
            | Self::Fingerprint
            | Self::Voiceprint
            | Self::RetinaScan
            | Self::FacialGeometry => EntitySensitivity::Critical,

            // High: directly identifying
            Self::TaxId
            | Self::DriversLicense
            | Self::PersonName
            | Self::DateOfBirth
            | Self::EmailAddress
            | Self::PhoneNumber
            | Self::Address
            | Self::MedicalId
            | Self::InsuranceId
            | Self::PrescriptionId
            | Self::Iban
            | Self::CryptoAddress
            | Self::Face
            | Self::Signature => EntitySensitivity::High,

            // Medium: indirectly identifying
            Self::Age
            | Self::Demographic
            | Self::PostalCode
            | Self::IpAddress
            | Self::MacAddress
            | Self::DeviceId
            | Self::Username
            | Self::Coordinates
            | Self::GeolocationMetadata
            | Self::CardExpiry
            | Self::BankRouting
            | Self::SwiftCode
            | Self::VehicleId
            | Self::LicensePlate
            | Self::DateTime
            | Self::Handwriting => EntitySensitivity::Medium,

            // Low: quasi-public
            Self::Url
            | Self::Amount
            | Self::OrganizationName
            | Self::Logo
            | Self::Barcode => EntitySensitivity::Low,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn display_snake_case() {
        assert_eq!(EntityKind::GovernmentId.to_string(), "government_id");
        assert_eq!(EntityKind::PaymentCard.to_string(), "payment_card");
        assert_eq!(EntityKind::EmailAddress.to_string(), "email_address");
        assert_eq!(EntityKind::Fingerprint.to_string(), "fingerprint");
        assert_eq!(EntityKind::ApiKey.to_string(), "api_key");
        assert_eq!(EntityKind::Face.to_string(), "face");
    }

    #[test]
    fn parse_roundtrip() {
        let kind = EntityKind::from_str("fingerprint").unwrap();
        assert_eq!(kind, EntityKind::Fingerprint);
        assert_eq!(kind.to_string(), "fingerprint");
    }

    #[test]
    fn serde_roundtrip() {
        let kind = EntityKind::ApiKey;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"api_key\"");
        let back: EntityKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, kind);
    }

    #[test]
    fn category_pii() {
        assert_eq!(EntityKind::GovernmentId.category(), EntityCategory::Pii);
        assert_eq!(EntityKind::PersonName.category(), EntityCategory::Pii);
        assert_eq!(EntityKind::Address.category(), EntityCategory::Pii);
    }

    #[test]
    fn category_financial() {
        assert_eq!(EntityKind::PaymentCard.category(), EntityCategory::Financial);
        assert_eq!(EntityKind::Iban.category(), EntityCategory::Financial);
    }

    #[test]
    fn category_phi() {
        assert_eq!(EntityKind::MedicalId.category(), EntityCategory::Phi);
        assert_eq!(EntityKind::PrescriptionId.category(), EntityCategory::Phi);
    }

    #[test]
    fn category_credentials() {
        assert_eq!(EntityKind::Password.category(), EntityCategory::Credentials);
        assert_eq!(EntityKind::ApiKey.category(), EntityCategory::Credentials);
        assert_eq!(EntityKind::AuthToken.category(), EntityCategory::Credentials);
        assert_eq!(EntityKind::PrivateKey.category(), EntityCategory::Credentials);
    }

    #[test]
    fn category_biometric() {
        assert_eq!(EntityKind::Fingerprint.category(), EntityCategory::Biometric);
        assert_eq!(EntityKind::Voiceprint.category(), EntityCategory::Biometric);
        assert_eq!(EntityKind::RetinaScan.category(), EntityCategory::Biometric);
        assert_eq!(EntityKind::FacialGeometry.category(), EntityCategory::Biometric);
        assert_eq!(EntityKind::Face.category(), EntityCategory::Biometric);
    }

    #[test]
    fn sensitivity_critical() {
        assert_eq!(EntityKind::GovernmentId.sensitivity(), EntitySensitivity::Critical);
        assert_eq!(EntityKind::PaymentCard.sensitivity(), EntitySensitivity::Critical);
        assert_eq!(EntityKind::Fingerprint.sensitivity(), EntitySensitivity::Critical);
        assert_eq!(EntityKind::Password.sensitivity(), EntitySensitivity::Critical);
    }

    #[test]
    fn sensitivity_high() {
        assert_eq!(EntityKind::PersonName.sensitivity(), EntitySensitivity::High);
        assert_eq!(EntityKind::EmailAddress.sensitivity(), EntitySensitivity::High);
        assert_eq!(EntityKind::MedicalId.sensitivity(), EntitySensitivity::High);
    }

    #[test]
    fn sensitivity_medium() {
        assert_eq!(EntityKind::Age.sensitivity(), EntitySensitivity::Medium);
        assert_eq!(EntityKind::IpAddress.sensitivity(), EntitySensitivity::Medium);
        assert_eq!(EntityKind::PostalCode.sensitivity(), EntitySensitivity::Medium);
    }

    #[test]
    fn sensitivity_low() {
        assert_eq!(EntityKind::Url.sensitivity(), EntitySensitivity::Low);
        assert_eq!(EntityKind::OrganizationName.sensitivity(), EntitySensitivity::Low);
    }

    #[test]
    fn sensitivity_ordering() {
        assert!(EntityKind::GovernmentId.sensitivity() > EntityKind::PersonName.sensitivity());
        assert!(EntityKind::PersonName.sensitivity() > EntityKind::Age.sensitivity());
        assert!(EntityKind::Age.sensitivity() > EntityKind::Url.sensitivity());
    }

    #[test]
    fn entity_kind_is_copy() {
        let a = EntityKind::Fingerprint;
        let b = a;
        assert_eq!(a, b);
    }
}

//! Concrete entity kind enumeration.
//!
//! [`EntityKind`] enumerates the types of sensitive data the platform
//! can detect or redact.  Each variant maps to a stable `snake_case`
//! string for serialization and display.
//!
//! Every variant also maps to:
//! - an [`EntityCategory`] via [`EntityKind::category`],
//! - an [`EntitySensitivity`] via [`EntityKind::sensitivity`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::category::EntityCategory;
use super::sensitivity::EntitySensitivity;

/// Specific kind of sensitive entity detected or targeted for redaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[derive(EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum EntityKind {
    // Personal identity
    /// Person name (full, first, or last).
    PersonName,
    /// Date of birth.
    DateOfBirth,
    /// Government-issued identification number (SSN, SIN, Aadhaar, national ID, etc.).
    GovernmentId,
    /// Tax identification number (ITIN, EIN, TIN, etc.).
    TaxId,
    /// Driver's license number.
    DriversLicense,
    /// Passport number.
    PassportNumber,
    /// National insurance or social-security equivalent (NI, BSN, AHVN, etc.).
    NationalInsuranceNumber,
    /// Vehicle identification number (VIN).
    VehicleId,
    /// License plate number.
    LicensePlate,

    // Contact information
    /// Email address.
    EmailAddress,
    /// Phone number.
    PhoneNumber,
    /// Physical or mailing address.
    Address,
    /// Postal or ZIP code.
    PostalCode,
    /// URL or hyperlink.
    Url,

    // Demographic
    /// Age value.
    Age,
    /// Gender identity.
    Gender,
    /// Racial or ethnic background.
    Ethnicity,
    /// Religious affiliation.
    Religion,
    /// Nationality.
    Nationality,
    /// Citizenship status.
    Citizenship,
    /// Language or dialect spoken.
    Language,

    // Financial
    /// Payment card number (credit or debit).
    PaymentCard,
    /// Payment card security code (CVV/CVC).
    CardSecurityCode,
    /// Payment card expiration date.
    CardExpiry,
    /// Bank account number.
    BankAccount,
    /// Bank routing or transit number.
    BankRouting,
    /// International Bank Account Number (IBAN).
    Iban,
    /// SWIFT / BIC code.
    SwiftCode,
    /// Cryptocurrency wallet address.
    CryptoAddress,
    /// Monetary amount.
    Amount,

    // Health
    /// Medical or patient identifier.
    MedicalId,
    /// Insurance policy number.
    InsuranceId,
    /// Prescription number.
    PrescriptionId,
    /// Medical diagnosis or condition.
    Diagnosis,
    /// Drug or medication name in a patient context.
    Medication,

    // Biometric
    /// Fingerprint template or minutiae data.
    Fingerprint,
    /// Voiceprint or speaker embedding.
    Voiceprint,
    /// Retina or iris scan data.
    RetinaScan,
    /// Facial geometry or face embedding (not a photo: see [`Face`](Self::Face)).
    FacialGeometry,

    // Credentials
    /// Password or passphrase.
    Password,
    /// API key.
    ApiKey,
    /// Authentication or session token.
    AuthToken,
    /// Private cryptographic key.
    PrivateKey,

    // Network and device identifiers
    /// IP address (v4 or v6).
    IpAddress,
    /// MAC (hardware) address.
    MacAddress,
    /// Device identifier (IMEI, IDFA, etc.).
    DeviceId,
    /// Username or online handle.
    Username,

    // Location
    /// GPS coordinates (latitude / longitude).
    Coordinates,
    /// Geolocation metadata (EXIF, cell tower, etc.).
    GeolocationMetadata,

    // Visual
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

    // Organizational
    /// Company or institution name.
    OrganizationName,
    /// Internal division or department name.
    DepartmentName,
    /// Physical facility name (hospital, office, school).
    FacilityName,
    /// Legal or administrative case identifier.
    CaseNumber,
    /// Internal reference number (invoice, contract, PO, employee number, membership ID).
    InternalId,

    // Temporal
    /// Date, time, or datetime value.
    DateTime,
}

impl EntityKind {
    /// Returns the [`EntityCategory`] this entity kind belongs to.
    pub fn category(&self) -> EntityCategory {
        match self {
            // Personal identity
            Self::PersonName
            | Self::DateOfBirth
            | Self::GovernmentId
            | Self::TaxId
            | Self::DriversLicense
            | Self::PassportNumber
            | Self::NationalInsuranceNumber
            | Self::VehicleId
            | Self::LicensePlate => EntityCategory::PersonalIdentity,

            // Contact
            Self::EmailAddress
            | Self::PhoneNumber
            | Self::Address
            | Self::PostalCode
            | Self::Url => EntityCategory::ContactInfo,

            // Demographic
            Self::Age
            | Self::Gender
            | Self::Ethnicity
            | Self::Religion
            | Self::Nationality
            | Self::Citizenship
            | Self::Language => EntityCategory::Demographic,

            // Financial
            Self::PaymentCard
            | Self::CardSecurityCode
            | Self::CardExpiry
            | Self::BankAccount
            | Self::BankRouting
            | Self::Iban
            | Self::SwiftCode
            | Self::CryptoAddress
            | Self::Amount => EntityCategory::Financial,

            // Health
            Self::MedicalId
            | Self::InsuranceId
            | Self::PrescriptionId
            | Self::Diagnosis
            | Self::Medication => EntityCategory::Health,

            // Biometric
            Self::Fingerprint | Self::Voiceprint | Self::RetinaScan | Self::FacialGeometry => {
                EntityCategory::Biometric
            }

            // Credentials
            Self::Password | Self::ApiKey | Self::AuthToken | Self::PrivateKey => {
                EntityCategory::Credentials
            }

            // Network
            Self::IpAddress | Self::MacAddress | Self::DeviceId | Self::Username => {
                EntityCategory::NetworkIdentifier
            }

            // Location
            Self::Coordinates | Self::GeolocationMetadata => EntityCategory::Location,

            // Visual
            Self::Face | Self::Handwriting | Self::Signature | Self::Logo | Self::Barcode => {
                EntityCategory::Visual
            }

            // Organizational
            Self::OrganizationName
            | Self::DepartmentName
            | Self::FacilityName
            | Self::CaseNumber
            | Self::InternalId => EntityCategory::Organizational,

            // Temporal (grouped under PersonalIdentity: bare dates most
            // commonly appear alongside personal data and are regulated
            // as PII by GDPR/CCPA)
            Self::DateTime => EntityCategory::PersonalIdentity,
        }
    }

    /// Returns the default [`EntitySensitivity`] for this entity kind.
    pub fn sensitivity(&self) -> EntitySensitivity {
        match self {
            // Critical: irrevocable identifiers, secrets, biometrics
            Self::GovernmentId
            | Self::PassportNumber
            | Self::NationalInsuranceNumber
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
            | Self::Diagnosis
            | Self::Medication
            | Self::Iban
            | Self::CryptoAddress
            | Self::Face
            | Self::Signature
            | Self::Coordinates => EntitySensitivity::High,

            // Medium: indirectly identifying
            Self::Age
            | Self::Gender
            | Self::Ethnicity
            | Self::Religion
            | Self::Nationality
            | Self::Citizenship
            | Self::Language
            | Self::PostalCode
            | Self::IpAddress
            | Self::MacAddress
            | Self::DeviceId
            | Self::Username
            | Self::CardExpiry
            | Self::BankRouting
            | Self::SwiftCode
            | Self::VehicleId
            | Self::LicensePlate
            | Self::GeolocationMetadata
            | Self::DateTime
            | Self::Handwriting
            | Self::CaseNumber
            | Self::InternalId => EntitySensitivity::Medium,

            // Low: quasi-public or context-dependent
            Self::Url
            | Self::Amount
            | Self::OrganizationName
            | Self::DepartmentName
            | Self::FacilityName
            | Self::Logo
            | Self::Barcode => EntitySensitivity::Low,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

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
    fn category_personal_identity() {
        assert_eq!(
            EntityKind::GovernmentId.category(),
            EntityCategory::PersonalIdentity
        );
        assert_eq!(
            EntityKind::PersonName.category(),
            EntityCategory::PersonalIdentity
        );
        assert_eq!(
            EntityKind::DateOfBirth.category(),
            EntityCategory::PersonalIdentity
        );
    }

    #[test]
    fn category_contact_info() {
        assert_eq!(
            EntityKind::EmailAddress.category(),
            EntityCategory::ContactInfo
        );
        assert_eq!(EntityKind::Address.category(), EntityCategory::ContactInfo);
    }

    #[test]
    fn category_demographic() {
        assert_eq!(EntityKind::Gender.category(), EntityCategory::Demographic);
        assert_eq!(
            EntityKind::Ethnicity.category(),
            EntityCategory::Demographic
        );
        assert_eq!(EntityKind::Religion.category(), EntityCategory::Demographic);
    }

    #[test]
    fn category_financial() {
        assert_eq!(
            EntityKind::PaymentCard.category(),
            EntityCategory::Financial
        );
        assert_eq!(EntityKind::Iban.category(), EntityCategory::Financial);
    }

    #[test]
    fn category_health() {
        assert_eq!(EntityKind::MedicalId.category(), EntityCategory::Health);
        assert_eq!(EntityKind::Diagnosis.category(), EntityCategory::Health);
        assert_eq!(EntityKind::Medication.category(), EntityCategory::Health);
    }

    #[test]
    fn category_credentials() {
        assert_eq!(EntityKind::Password.category(), EntityCategory::Credentials);
        assert_eq!(EntityKind::ApiKey.category(), EntityCategory::Credentials);
    }

    #[test]
    fn category_biometric() {
        assert_eq!(
            EntityKind::Fingerprint.category(),
            EntityCategory::Biometric
        );
        assert_eq!(EntityKind::Voiceprint.category(), EntityCategory::Biometric);
        assert_eq!(EntityKind::RetinaScan.category(), EntityCategory::Biometric);
        assert_eq!(EntityKind::Face.category(), EntityCategory::Visual);
    }

    #[test]
    fn category_organizational() {
        assert_eq!(
            EntityKind::OrganizationName.category(),
            EntityCategory::Organizational
        );
        assert_eq!(
            EntityKind::CaseNumber.category(),
            EntityCategory::Organizational
        );
        assert_eq!(
            EntityKind::InternalId.category(),
            EntityCategory::Organizational
        );
    }

    #[test]
    fn sensitivity_critical() {
        assert_eq!(
            EntityKind::GovernmentId.sensitivity(),
            EntitySensitivity::Critical
        );
        assert_eq!(
            EntityKind::PaymentCard.sensitivity(),
            EntitySensitivity::Critical
        );
        assert_eq!(
            EntityKind::Fingerprint.sensitivity(),
            EntitySensitivity::Critical
        );
        assert_eq!(
            EntityKind::Password.sensitivity(),
            EntitySensitivity::Critical
        );
    }

    #[test]
    fn sensitivity_high() {
        assert_eq!(
            EntityKind::PersonName.sensitivity(),
            EntitySensitivity::High
        );
        assert_eq!(
            EntityKind::EmailAddress.sensitivity(),
            EntitySensitivity::High
        );
        assert_eq!(EntityKind::MedicalId.sensitivity(), EntitySensitivity::High);
        assert_eq!(EntityKind::Diagnosis.sensitivity(), EntitySensitivity::High);
    }

    #[test]
    fn sensitivity_medium() {
        assert_eq!(EntityKind::Age.sensitivity(), EntitySensitivity::Medium);
        assert_eq!(
            EntityKind::IpAddress.sensitivity(),
            EntitySensitivity::Medium
        );
        assert_eq!(
            EntityKind::PostalCode.sensitivity(),
            EntitySensitivity::Medium
        );
    }

    #[test]
    fn sensitivity_low() {
        assert_eq!(EntityKind::Url.sensitivity(), EntitySensitivity::Low);
        assert_eq!(
            EntityKind::OrganizationName.sensitivity(),
            EntitySensitivity::Low
        );
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

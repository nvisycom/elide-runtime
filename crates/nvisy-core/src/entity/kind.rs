//! Concrete entity kind enumeration.
//!
//! [`EntityKind`] enumerates the types of sensitive data the platform
//! can detect or redact.  Each variant maps to a stable `snake_case`
//! string for serialization and display, and to an [`EntityCategory`]
//! via [`EntityKind::category`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use super::category::EntityCategory;

/// Specific kind of sensitive entity detected or targeted for redaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Display)]
#[derive(EnumIter, EnumString, Serialize, Deserialize, JsonSchema)]
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
    /// Currency name or ISO 4217 code (USD, US Dollar, EUR, BTC,
    /// Bitcoin, …). Distinct from a concrete [`Amount`].
    ///
    /// [`Amount`]: Self::Amount
    Currency,
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
    /// Facial geometry or face embedding (not a photo: see [`Face`]).
    ///
    /// [`Face`]: Self::Face
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

    // General-purpose NER labels (commonly emitted by zero-shot
    // models like GLiNER): not strictly PII but useful to flag for
    // policy routing, redaction overrides, or downstream
    // structuring.
    /// Event reference (conferences, weddings, public happenings).
    Event,
    /// Occupation, role, or job title.
    Occupation,
    /// Product, service, or model name.
    Product,
    /// Numeric quantity or measurement (distinct from monetary
    /// [`Amount`]).
    ///
    /// [`Amount`]: Self::Amount
    Quantity,

    /// Fallback kind for entities a recognizer flagged as sensitive
    /// but could not classify into a more specific kind. Pairs with
    /// [`EntityCategory::Unresolved`].
    #[default]
    Unresolved,
}

impl EntityKind {
    /// Every defined [`EntityKind`] variant, in declaration order.
    ///
    /// Use with combinators to build category-filtered allowlists
    /// without enumerating variants by hand:
    ///
    /// ```ignore
    /// let text_kinds: Vec<EntityKind> = EntityKind::all()
    ///     .filter(|k| !k.is_biometric() && !k.is_visual())
    ///     .collect();
    /// ```
    pub fn all() -> impl Iterator<Item = EntityKind> {
        <Self as IntoEnumIterator>::iter()
    }

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
            | Self::Currency
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

            // General-purpose
            Self::Event | Self::Occupation | Self::Product | Self::Quantity => {
                EntityCategory::GeneralPurpose
            }

            Self::Unresolved => EntityCategory::Unresolved,
        }
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::PersonalIdentity`].
    #[must_use]
    pub fn is_personal_identity(&self) -> bool {
        self.category() == EntityCategory::PersonalIdentity
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::ContactInfo`].
    #[must_use]
    pub fn is_contact_info(&self) -> bool {
        self.category() == EntityCategory::ContactInfo
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::Demographic`].
    #[must_use]
    pub fn is_demographic(&self) -> bool {
        self.category() == EntityCategory::Demographic
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::Financial`].
    #[must_use]
    pub fn is_financial(&self) -> bool {
        self.category() == EntityCategory::Financial
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::Health`].
    #[must_use]
    pub fn is_health(&self) -> bool {
        self.category() == EntityCategory::Health
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::Biometric`].
    #[must_use]
    pub fn is_biometric(&self) -> bool {
        self.category() == EntityCategory::Biometric
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::Credentials`].
    #[must_use]
    pub fn is_credentials(&self) -> bool {
        self.category() == EntityCategory::Credentials
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::NetworkIdentifier`].
    #[must_use]
    pub fn is_network_identifier(&self) -> bool {
        self.category() == EntityCategory::NetworkIdentifier
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::Location`].
    #[must_use]
    pub fn is_location(&self) -> bool {
        self.category() == EntityCategory::Location
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::Visual`].
    #[must_use]
    pub fn is_visual(&self) -> bool {
        self.category() == EntityCategory::Visual
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::Organizational`].
    #[must_use]
    pub fn is_organizational(&self) -> bool {
        self.category() == EntityCategory::Organizational
    }

    /// Convenience predicate: this kind belongs to
    /// [`EntityCategory::GeneralPurpose`].
    #[must_use]
    pub fn is_general_purpose(&self) -> bool {
        self.category() == EntityCategory::GeneralPurpose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

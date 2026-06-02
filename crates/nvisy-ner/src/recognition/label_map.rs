//! [`LabelMap`]: model-label → [`EntityKind`] translation table.
//!
//! Lives client-side as part of
//! [`NerModelConfiguration`]. Lets
//! the adapter recognizers (`NlpRecognizer`, `GlinerRecognizer`)
//! consume raw model labels uniformly regardless of which backend
//! produced them — swap backends without re-implementing
//! translation.
//!
//! The map is bidirectional in spirit (look up an `EntityKind` to
//! find the canonical label a backend should be asked for) but the
//! primary path is label→kind. The reverse lookup is a linear
//! scan; if a future backend needs frequent reverse lookups we'll
//! cache both directions.
//!
//! [`NerModelConfiguration`]: super::NerModelConfiguration

use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;

use nvisy_ontology::entity::EntityKind;
use serde::{Deserialize, Serialize};

/// Translation table from raw model labels to canonical
/// [`EntityKind`] values.
///
/// The default ([`LabelMap::canonical`]) maps every `EntityKind`'s
/// snake_case string form to itself, so backends that already
/// return canonical labels (the Bento `inference-gliner` today)
/// pass through unchanged. Custom backends register their own
/// model-specific labels via [`with_entry`].
///
/// [`with_entry`]: Self::with_entry
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LabelMap {
    entries: HashMap<String, EntityKind>,
}

impl LabelMap {
    /// Empty map. Backends with no recognizable labels see every
    /// span dropped — typically you want
    /// [`canonical`] instead.
    ///
    /// [`canonical`]: Self::canonical
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Identity map: every [`EntityKind`] mapped to its own
    /// canonical snake_case label. Use when the backend already
    /// returns canonical labels.
    #[must_use]
    pub fn canonical() -> Self {
        let mut entries = HashMap::new();
        // Discovery via from_str round-trip: enumerate the labels
        // we know about. The list mirrors the EntityKind enum.
        // Kept lazy — adding a variant doesn't break the map (the
        // variant just isn't recognized until added here).
        for label in [
            "person_name",
            "date_of_birth",
            "government_id",
            "tax_id",
            "drivers_license",
            "passport_number",
            "national_insurance_number",
            "vehicle_id",
            "license_plate",
            "email_address",
            "phone_number",
            "address",
            "postal_code",
            "url",
            "age",
            "gender",
            "ethnicity",
            "religion",
            "nationality",
            "citizenship",
            "language",
            "payment_card",
            "card_security_code",
            "card_expiry",
            "bank_account",
            "bank_routing",
            "iban",
            "swift_code",
            "crypto_address",
            "currency",
            "amount",
            "medical_id",
            "insurance_id",
            "prescription_id",
            "diagnosis",
            "medication",
            "fingerprint",
            "voiceprint",
            "retina_scan",
            "facial_geometry",
            "password",
            "api_key",
            "auth_token",
            "private_key",
            "ip_address",
            "mac_address",
            "device_id",
            "username",
            "coordinates",
            "geolocation_metadata",
            "face",
            "handwriting",
            "signature",
            "logo",
            "barcode",
            "organization_name",
            "department_name",
            "facility_name",
            "case_number",
            "internal_id",
            "date_time",
            "event",
            "occupation",
            "product",
            "quantity",
            "unresolved",
        ] {
            if let Ok(kind) = EntityKind::from_str(label) {
                entries.insert(label.to_owned(), kind);
            }
        }
        Self { entries }
    }

    /// Register one label→kind entry. Last write wins on duplicate
    /// labels.
    #[must_use]
    pub fn with_entry(mut self, label: impl Into<Cow<'static, str>>, kind: EntityKind) -> Self {
        self.entries.insert(label.into().into_owned(), kind);
        self
    }

    /// Register many entries.
    #[must_use]
    pub fn with_entries<I, S>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = (S, EntityKind)>,
        S: Into<String>,
    {
        for (label, kind) in entries {
            self.entries.insert(label.into(), kind);
        }
        self
    }

    /// Look up a raw label. `None` when not registered.
    #[must_use]
    pub fn lookup(&self, label: &str) -> Option<EntityKind> {
        self.entries.get(label).copied()
    }

    /// Find a label string that maps to `kind`. Linear scan;
    /// returns the first match. Used by zero-shot backends that
    /// need to format requested-kinds as raw labels for the
    /// service.
    #[must_use]
    pub fn label_for(&self, kind: EntityKind) -> Option<&str> {
        self.entries
            .iter()
            .find_map(|(label, k)| (*k == kind).then_some(label.as_str()))
    }

    /// Number of registered entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_map_resolves_known_labels() {
        let map = LabelMap::canonical();
        assert_eq!(map.lookup("email_address"), Some(EntityKind::EmailAddress));
        assert_eq!(map.lookup("ssn"), None);
    }

    #[test]
    fn custom_entries_override_canonical() {
        let map = LabelMap::canonical().with_entry("PER", EntityKind::PersonName);
        assert_eq!(map.lookup("PER"), Some(EntityKind::PersonName));
    }

    #[test]
    fn label_for_round_trips() {
        let map = LabelMap::canonical();
        assert_eq!(
            map.label_for(EntityKind::EmailAddress),
            Some("email_address")
        );
    }
}

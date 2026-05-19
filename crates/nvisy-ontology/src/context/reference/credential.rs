//! Credential reference data for secret detection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Classification of credential secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CredentialKind {
    /// API key.
    ApiKey,
    /// OAuth access or refresh token.
    OauthToken,
    /// Password or passphrase.
    Password,
    /// Private key (SSH, TLS, etc.).
    PrivateKey,
    /// Other credential type.
    Other,
}

/// A reference credential for detecting leaked secrets.
///
/// The `value` field is intentionally excluded from serialization to
/// prevent plaintext secrets from appearing in logs or API responses.
/// It is only accepted during deserialization (ingest).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CredentialData {
    /// The credential value (API key, token, etc.).
    ///
    /// Excluded from serialization output to prevent leaking secrets.
    /// Round-trip deserialization yields an empty string.
    #[serde(default, skip_serializing)]
    pub value: String,
    /// Classification of this credential.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_kind: Option<CredentialKind>,
    /// Service or provider this credential belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_is_not_serialized() {
        let cred = CredentialData {
            value: "sk-secret-token".to_owned(),
            credential_kind: Some(CredentialKind::ApiKey),
            provider: Some("openai".to_owned()),
        };
        let json = serde_json::to_string(&cred).unwrap();
        assert!(
            !json.contains("sk-secret-token"),
            "serialized JSON must not contain plaintext value, got: {json}"
        );
        assert!(json.contains(r#""credentialKind":"api_key""#));
    }

    #[test]
    fn round_trip_defaults_value_to_empty() {
        // Serialize without value, then deserialize: value must default to empty string.
        let cred = CredentialData {
            value: "secret".to_owned(),
            credential_kind: None,
            provider: None,
        };
        let json = serde_json::to_string(&cred).unwrap();
        let back: CredentialData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.value, "");
    }
}

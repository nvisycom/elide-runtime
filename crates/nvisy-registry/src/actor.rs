use std::fmt;

use jiff::Zoned;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::{Error, ErrorKind, Result};

/// UUID-based actor identity.
///
/// Every resource stored in the [`Registry`](crate::Registry) is scoped
/// by an `ActorId`. Composite keys (`actor_id ++ resource_id`) ensure
/// that list and read operations are inherently isolated per actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ActorId(Uuid);

impl ActorId {
    /// Create a new actor identity with a fresh UUIDv7.
    #[must_use]
    pub fn new() -> Self {
        let now = Zoned::now();
        let timestamp = uuid::Timestamp::from_unix(
            uuid::NoContext,
            now.timestamp().as_second().unsigned_abs(),
            now.timestamp().subsec_nanosecond().unsigned_abs(),
        );
        Self(Uuid::new_v7(timestamp))
    }

    /// Create an `ActorId` from an existing UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Parse an `ActorId` from a UUID string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not a valid UUID format.
    pub fn parse(s: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(s).map_err(|err| {
            Error::new(ErrorKind::Validation, format!("Invalid actor UUID: {err}"))
        })?;
        Ok(Self(uuid))
    }

    /// Get the underlying UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for ActorId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ActorId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<ActorId> for Uuid {
    fn from(actor: ActorId) -> Self {
        actor.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_generates_v7() {
        let actor = ActorId::new();
        assert_eq!(actor.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn from_uuid_roundtrip() {
        let uuid = Uuid::new_v4();
        let actor = ActorId::from_uuid(uuid);
        assert_eq!(actor.as_uuid(), uuid);
    }

    #[test]
    fn parse_roundtrip() {
        let actor = ActorId::new();
        let parsed = ActorId::parse(&actor.to_string()).unwrap();
        assert_eq!(actor, parsed);
    }

    #[test]
    fn parse_invalid() {
        let err = ActorId::parse("not-a-uuid").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Validation);
    }

    #[test]
    fn display_matches_uuid() {
        let actor = ActorId::new();
        assert_eq!(actor.to_string(), actor.as_uuid().to_string());
    }

    #[test]
    fn serde_roundtrip() {
        let actor = ActorId::new();
        let json = serde_json::to_string(&actor).unwrap();
        let deserialized: ActorId = serde_json::from_str(&json).unwrap();
        assert_eq!(actor, deserialized);
    }
}

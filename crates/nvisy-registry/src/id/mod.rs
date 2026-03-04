use std::fmt;

use jiff::Zoned;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::{Error, ErrorKind, Result};

macro_rules! define_id {
    (
        $(#[$meta:meta])*
        $name:ident, $label:literal
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[derive(Serialize, Deserialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Create a new identifier with a fresh UUIDv7.
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

            /// Create from an existing UUID.
            #[must_use]
            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Parse from a UUID string.
            ///
            /// # Errors
            ///
            /// Returns an error if the string is not a valid UUID format.
            pub fn parse(s: &str) -> Result<Self> {
                let uuid = Uuid::parse_str(s).map_err(|err| {
                    Error::new(
                        ErrorKind::Validation,
                        format!(concat!("Invalid ", $label, " UUID: {}"), err),
                    )
                })?;
                Ok(Self(uuid))
            }

            /// Get the underlying UUID.
            #[must_use]
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

define_id! {
    /// UUID-based actor identity.
    ///
    /// Every resource stored in the [`Registry`](crate::Registry) is scoped
    /// by an `ActorId`. Composite keys (`actor_id ++ resource_id`) ensure
    /// that list and read operations are inherently isolated per actor.
    ActorId, "actor"
}

define_id! {
    /// UUID-based content identifier.
    ///
    /// References a content entry (file) in the [`Registry`](crate::Registry).
    ContentId, "content"
}

define_id! {
    /// UUID-based context identifier.
    ///
    /// References a context entry in the [`Registry`](crate::Registry).
    ContextId, "context"
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! id_tests {
        ($name:ident, $mod_name:ident) => {
            mod $mod_name {
                use super::*;

                #[test]
                fn new_generates_v7() {
                    let id = $name::new();
                    assert_eq!(id.as_uuid().get_version_num(), 7);
                }

                #[test]
                fn from_uuid_roundtrip() {
                    let uuid = Uuid::new_v4();
                    let id = $name::from_uuid(uuid);
                    assert_eq!(id.as_uuid(), uuid);
                }

                #[test]
                fn parse_roundtrip() {
                    let id = $name::new();
                    let parsed = $name::parse(&id.to_string()).unwrap();
                    assert_eq!(id, parsed);
                }

                #[test]
                fn parse_invalid() {
                    let err = $name::parse("not-a-uuid").unwrap_err();
                    assert_eq!(err.kind, ErrorKind::Validation);
                }

                #[test]
                fn display_matches_uuid() {
                    let id = $name::new();
                    assert_eq!(id.to_string(), id.as_uuid().to_string());
                }

                #[test]
                fn serde_roundtrip() {
                    let id = $name::new();
                    let json = serde_json::to_string(&id).unwrap();
                    let deserialized: $name = serde_json::from_str(&json).unwrap();
                    assert_eq!(id, deserialized);
                }
            }
        };
    }

    id_tests!(ActorId, actor_id);
    id_tests!(ContentId, content_id);
    id_tests!(ContextId, context_id);
}

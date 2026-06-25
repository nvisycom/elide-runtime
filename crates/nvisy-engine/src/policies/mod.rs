//! Policy resource API over the engine's fjall registry.
//!
//! Surfaced as the [`PolicyRegistry`] extension trait on
//! [`RegistryHandle`] so resource calls flow through the same
//! handle every other engine entry point takes —
//! `handle.get_policy(...)`, `handle.put_policy(...)`, etc.
//!
//! Policies are immutable per `(actor_id, policy_id, version)`.
//! To "edit" a policy, write a new version. The keyspace stores
//! every version side-by-side so old runs can replay against the
//! exact bytes they were submitted under.
//!
//! Surface:
//!
//! - [`put_policy`](PolicyRegistry::put_policy) — write a new
//!   version (rejects writes that would overwrite an existing
//!   `(actor, id, version)`).
//! - [`get_policy`](PolicyRegistry::get_policy) — read one
//!   specific version.
//! - [`latest_policy`](PolicyRegistry::latest_policy) — find the
//!   highest version for `(actor, id)`.
//! - [`list_policies`](PolicyRegistry::list_policies) — list
//!   `(id, version)` pairs for an actor.
//! - [`delete_policy`](PolicyRegistry::delete_policy) — remove
//!   one specific version.

use nvisy_core::policy::Policy;
use nvisy_core::{Error, Result};
use semver::Version;
use uuid::Uuid;

use crate::registry::{PagedResult, RegistryHandle, VersionedKey, blocking, not_found};

const COMPONENT: &str = "policies";
const KIND: &str = "policy";

/// Extension trait adding policy-resource CRUD to
/// [`RegistryHandle`].
///
/// Implemented for `RegistryHandle` itself; bring the trait into
/// scope (`use nvisy_engine::PolicyRegistry;`) to call its
/// methods.
pub trait PolicyRegistry {
    /// Write a new policy version. Fails with
    /// [`ErrorKind::Conflict`] when an entry already exists at
    /// `(actor_id, policy.id, version)`.
    ///
    /// [`ErrorKind::Conflict`]: nvisy_core::ErrorKind::Conflict
    fn put_policy(
        &self,
        actor_id: Uuid,
        policy: &Policy,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Read a specific policy version. Returns
    /// [`ErrorKind::NotFound`] when no entry exists at
    /// `(actor_id, policy_id, version)`.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    fn get_policy(
        &self,
        actor_id: Uuid,
        policy_id: Uuid,
        version: Version,
    ) -> impl Future<Output = Result<Policy>> + Send;

    /// Read the highest-version policy for
    /// `(actor_id, policy_id)`. Returns
    /// [`ErrorKind::NotFound`] when no version exists.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    fn latest_policy(
        &self,
        actor_id: Uuid,
        policy_id: Uuid,
    ) -> impl Future<Output = Result<Policy>> + Send;

    /// List every `(policy_id, version)` pair stored for
    /// `actor_id`. Versions land in lex order (matches semver
    /// order for the encoded `(major, minor, patch)` triple).
    fn list_policies(
        &self,
        actor_id: Uuid,
    ) -> impl Future<Output = Result<PagedResult<(Uuid, Version)>>> + Send;

    /// Remove one specific policy version. Returns
    /// [`ErrorKind::NotFound`] if the entry was already absent.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    fn delete_policy(
        &self,
        actor_id: Uuid,
        policy_id: Uuid,
        version: Version,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl PolicyRegistry for RegistryHandle {
    async fn put_policy(&self, actor_id: Uuid, policy: &Policy) -> Result<()> {
        let key = VersionedKey::new(actor_id, policy.id, &policy.version);
        let value = serde_json::to_vec(policy)?;
        let policies = self.policies().clone();
        let policy_id = policy.id;
        let version = policy.version.clone();
        blocking(move || {
            if policies.contains_key(key.as_bytes()).map_err(fjall_err)? {
                return Err(Error::conflict(
                    format!(
                        "policy already exists at actor_id={actor_id}, policy_id={policy_id}, \
                         version={version} — bump the version to write again",
                    ),
                    COMPONENT,
                ));
            }
            policies.insert(key.as_bytes(), value).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }

    async fn get_policy(
        &self,
        actor_id: Uuid,
        policy_id: Uuid,
        version: Version,
    ) -> Result<Policy> {
        let key = VersionedKey::new(actor_id, policy_id, &version);
        let policies = self.policies().clone();
        blocking(move || {
            let value = policies
                .get(key.as_bytes())
                .map_err(fjall_err)?
                .ok_or_else(|| not_found(KIND, actor_id, policy_id))?;
            serde_json::from_slice(&value).map_err(Into::into)
        })
        .await
    }

    async fn latest_policy(&self, actor_id: Uuid, policy_id: Uuid) -> Result<Policy> {
        let policies = self.policies().clone();
        blocking(move || {
            let prefix = VersionedKey::prefix(actor_id, policy_id);
            let guard = policies
                .prefix(prefix)
                .next_back()
                .ok_or_else(|| not_found(KIND, actor_id, policy_id))?;
            let (_, value) = guard.into_inner().map_err(fjall_err)?;
            serde_json::from_slice(&value).map_err(Into::into)
        })
        .await
    }

    async fn list_policies(&self, actor_id: Uuid) -> Result<PagedResult<(Uuid, Version)>> {
        let policies = self.policies().clone();
        blocking(move || {
            let prefix = VersionedKey::actor_prefix(actor_id);
            let mut items: Vec<(Uuid, Version)> = Vec::new();
            for guard in policies.prefix(prefix) {
                let key = guard.key().map_err(fjall_err)?;
                let bytes = key.as_ref();
                let id = VersionedKey::resource_id_from_bytes(bytes)
                    .ok_or_else(|| Error::internal("unexpected policy key length", COMPONENT))?;
                let (major, minor, patch) = VersionedKey::version_from_bytes(bytes)
                    .ok_or_else(|| Error::internal("unexpected policy key length", COMPONENT))?;
                items.push((id, Version::new(major, minor, patch)));
            }
            let total = items.len();
            Ok(PagedResult { items, total })
        })
        .await
    }

    async fn delete_policy(&self, actor_id: Uuid, policy_id: Uuid, version: Version) -> Result<()> {
        let key = VersionedKey::new(actor_id, policy_id, &version);
        let policies = self.policies().clone();
        blocking(move || {
            if !policies.contains_key(key.as_bytes()).map_err(fjall_err)? {
                return Err(not_found(KIND, actor_id, policy_id));
            }
            policies.remove(key.as_bytes()).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }
}

fn fjall_err(err: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::internal("fjall operation failed", COMPONENT).with_source(err)
}

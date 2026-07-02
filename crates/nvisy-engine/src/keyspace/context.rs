//! Context resource API over the engine's fjall registry.
//!
//! Surfaced as the [`ContextRegistry`] extension trait on
//! [`RegistryHandle`]. Mirrors [`super::policy::PolicyRegistry`]
//! in shape: contexts are immutable per
//! `(actor_id, context_id, version)`. To "edit" a context, write
//! a new version. The keyspace stores every version side-by-side
//! so old runs can replay against the exact bytes they were
//! submitted under.

use std::error::Error as StdError;

use nvisy_core::{Error, Result};
use nvisy_schema::context::Context;
use semver::Version;
use uuid::Uuid;

use crate::registry::{PagedResult, RegistryHandle, VersionedKey, blocking, not_found};

const COMPONENT: &str = "contexts";
const KIND: &str = "context";

/// Extension trait adding context-resource CRUD to
/// [`RegistryHandle`].
///
/// Implemented for `RegistryHandle` itself; bring the trait into
/// scope (`use nvisy_engine::ContextRegistry;`) to call its
/// methods.
pub trait ContextRegistry {
    /// Write a new context version. Fails with
    /// [`ErrorKind::Conflict`] when an entry already exists at
    /// `(actor_id, context.id, version)`.
    ///
    /// [`ErrorKind::Conflict`]: nvisy_core::ErrorKind::Conflict
    fn put_context(
        &self,
        actor_id: Uuid,
        context: &Context,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Read a specific context version.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    fn get_context(
        &self,
        actor_id: Uuid,
        context_id: Uuid,
        version: Version,
    ) -> impl Future<Output = Result<Context>> + Send;

    /// Read the highest-version context for
    /// `(actor_id, context_id)`.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    fn latest_context(
        &self,
        actor_id: Uuid,
        context_id: Uuid,
    ) -> impl Future<Output = Result<Context>> + Send;

    /// List every `(context_id, version)` pair stored for
    /// `actor_id`.
    fn list_contexts(
        &self,
        actor_id: Uuid,
    ) -> impl Future<Output = Result<PagedResult<(Uuid, Version)>>> + Send;

    /// Remove one specific context version.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    fn delete_context(
        &self,
        actor_id: Uuid,
        context_id: Uuid,
        version: Version,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl ContextRegistry for RegistryHandle {
    async fn put_context(&self, actor_id: Uuid, context: &Context) -> Result<()> {
        let key = VersionedKey::new(actor_id, context.id, &context.version);
        let value = serde_json::to_vec(context)?;
        let contexts = self.contexts().clone();
        let context_id = context.id;
        let version = context.version.clone();
        blocking(move || {
            if contexts.contains_key(key.as_bytes()).map_err(fjall_err)? {
                return Err(Error::conflict(
                    format!(
                        "context already exists at actor_id={actor_id}, \
                         context_id={context_id}, version={version} — bump the version to \
                         write again",
                    ),
                    COMPONENT,
                ));
            }
            contexts.insert(key.as_bytes(), value).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }

    async fn get_context(
        &self,
        actor_id: Uuid,
        context_id: Uuid,
        version: Version,
    ) -> Result<Context> {
        let key = VersionedKey::new(actor_id, context_id, &version);
        let contexts = self.contexts().clone();
        blocking(move || {
            let value = contexts
                .get(key.as_bytes())
                .map_err(fjall_err)?
                .ok_or_else(|| not_found(KIND, actor_id, context_id))?;
            serde_json::from_slice(&value).map_err(Into::into)
        })
        .await
    }

    async fn latest_context(&self, actor_id: Uuid, context_id: Uuid) -> Result<Context> {
        let contexts = self.contexts().clone();
        blocking(move || {
            let prefix = VersionedKey::prefix(actor_id, context_id);
            let guard = contexts
                .prefix(prefix)
                .next_back()
                .ok_or_else(|| not_found(KIND, actor_id, context_id))?;
            let (_, value) = guard.into_inner().map_err(fjall_err)?;
            serde_json::from_slice(&value).map_err(Into::into)
        })
        .await
    }

    async fn list_contexts(&self, actor_id: Uuid) -> Result<PagedResult<(Uuid, Version)>> {
        let contexts = self.contexts().clone();
        blocking(move || {
            let prefix = VersionedKey::actor_prefix(actor_id);
            let mut items: Vec<(Uuid, Version)> = Vec::new();
            for guard in contexts.prefix(prefix) {
                let key = guard.key().map_err(fjall_err)?;
                let bytes = key.as_ref();
                let id = VersionedKey::resource_id_from_bytes(bytes)
                    .ok_or_else(|| Error::internal("unexpected context key length", COMPONENT))?;
                let (major, minor, patch) = VersionedKey::version_from_bytes(bytes)
                    .ok_or_else(|| Error::internal("unexpected context key length", COMPONENT))?;
                items.push((id, Version::new(major, minor, patch)));
            }
            let total = items.len();
            Ok(PagedResult { items, total })
        })
        .await
    }

    async fn delete_context(
        &self,
        actor_id: Uuid,
        context_id: Uuid,
        version: Version,
    ) -> Result<()> {
        let key = VersionedKey::new(actor_id, context_id, &version);
        let contexts = self.contexts().clone();
        blocking(move || {
            if !contexts.contains_key(key.as_bytes()).map_err(fjall_err)? {
                return Err(not_found(KIND, actor_id, context_id));
            }
            contexts.remove(key.as_bytes()).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }
}

fn fjall_err(err: impl StdError + Send + Sync + 'static) -> Error {
    Error::internal("fjall operation failed", COMPONENT).with_source(err)
}

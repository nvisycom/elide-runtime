//! [`Registry`]: actor-scoped content and policy store.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fjall::{Database, Keyspace};
use nvisy_codec::content::{Content, ContentMetadata, ContentSource};
use nvisy_core::Result;
use nvisy_core::modality::Text;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::composite_key::CompositeKey;
use super::content_handle::ContentHandle;
use super::fjall_ext::{FjallDatabaseExt, FjallKeyspaceExt, blocking, not_found};
use super::resource_cache::ResourceCache;
use crate::policy::Policy;
use crate::provenance::AnyAudit;

const TARGET: &str = "nvisy_document::ingestion::registry";

/// Actor-scoped content and policy store backed by fjall.
///
/// Cheaply cloneable (`Arc` internally).
#[derive(Clone)]
pub struct Registry {
    inner: Arc<RegistryInner>,
}

struct RegistryInner {
    base_dir: PathBuf,
    db: Database,
    content_ks: Keyspace,
    content_meta_ks: Keyspace,
    policies_ks: Keyspace,
    audits_ks: Keyspace,
    policy_cache: ResourceCache<Policy<Text>>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("base_dir", &self.inner.base_dir)
            .finish_non_exhaustive()
    }
}

impl Registry {
    /// Opens (or creates) the database at `path`.
    #[tracing::instrument(target = TARGET, name = "registry.open", fields(path = %path.as_ref().display()))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let base_dir = path.as_ref().to_path_buf();
        let db = Database::open_at(&base_dir)?;
        let content_ks = db.open_blob_keyspace("content")?;
        let content_meta_ks = db.open_keyspace("content_meta")?;
        let policies_ks = db.open_keyspace("policies")?;
        let audits_ks = db.open_keyspace("run_outputs")?;

        tracing::debug!(target: TARGET, "registry opened");
        Ok(Self {
            inner: Arc::new(RegistryInner {
                base_dir,
                db,
                content_ks,
                content_meta_ks,
                policies_ks,
                audits_ks,
                policy_cache: ResourceCache::new("policy"),
            }),
        })
    }

    /// Returns the base directory path.
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.inner.base_dir
    }

    /// Returns the shared policy cache.
    pub fn policy_cache(&self) -> &ResourceCache<Policy<Text>> {
        &self.inner.policy_cache
    }

    /// Registers content (bytes + metadata).
    #[tracing::instrument(target = TARGET, name = "registry.register_content", skip(self, content), fields(%actor_id))]
    pub async fn register_content(
        &self,
        actor_id: Uuid,
        content: Content,
    ) -> Result<ContentHandle> {
        let content_source = content.content_source();
        let key = CompositeKey::new(actor_id, content_source.as_uuid());
        let data = content.as_bytes().to_vec();

        let (content_data, content_metadata) = content.into_parts();
        let mut meta = content_metadata.unwrap_or_default();
        if meta.detected_content_type.is_none() {
            meta.detected_content_type = content_data.detect_mime();
        }
        if meta.size.is_none() {
            meta.size = Some(content_data.size() as u64);
        }
        if meta.sha256.is_none() {
            meta.sha256 = Some(content_data.sha256_hex());
        }

        let meta_bytes = serde_json::to_vec(&meta)?;
        let content_ks = self.inner.content_ks.clone();
        let meta_ks = self.inner.content_meta_ks.clone();
        let db = self.inner.db.clone();

        blocking(move || {
            content_ks.put(key, &data)?;
            meta_ks.put(key, &meta_bytes)?;
            db.sync()
        })
        .await?;

        tracing::trace!(target: TARGET, source_id = %content_source.as_uuid(), "content registered");
        Ok(ContentHandle::new(
            actor_id,
            content_source,
            self.inner.content_ks.clone(),
            self.inner.content_meta_ks.clone(),
        ))
    }

    /// Reads previously registered content.
    #[tracing::instrument(target = TARGET, name = "registry.read_content", skip(self), fields(%actor_id, %content_id))]
    pub async fn read_content(&self, actor_id: Uuid, content_id: Uuid) -> Result<ContentHandle> {
        let key = CompositeKey::new(actor_id, content_id);
        let ks = self.inner.content_ks.clone();
        let exists = blocking(move || ks.exists(key)).await?;
        if !exists {
            return Err(not_found("content", actor_id, content_id));
        }
        Ok(ContentHandle::new(
            actor_id,
            ContentSource::from(content_id),
            self.inner.content_ks.clone(),
            self.inner.content_meta_ks.clone(),
        ))
    }

    /// Removes a single content entry (data + metadata).
    /// Removes a content entry plus its metadata. Returns
    /// `NotFound` if no entry exists for the given actor/content
    /// pair.
    ///
    /// The check-then-delete sequence is not transactionally
    /// atomic: two concurrent calls for the same key race on
    /// `exists`, one wins, the other returns `NotFound`. That's
    /// the intended outcome (the second caller's view is "the key
    /// is gone"), so no synchronisation is needed beyond fjall's
    /// per-key linearisability.
    #[tracing::instrument(target = TARGET, name = "registry.unregister_content", skip(self), fields(%actor_id, %content_id))]
    pub async fn unregister_content(&self, actor_id: Uuid, content_id: Uuid) -> Result<()> {
        let key = CompositeKey::new(actor_id, content_id);
        let content_ks = self.inner.content_ks.clone();
        let meta_ks = self.inner.content_meta_ks.clone();
        let db = self.inner.db.clone();

        blocking(move || {
            if !content_ks.exists(key)? {
                return Err(not_found("content", actor_id, content_id));
            }
            content_ks.delete(key)?;
            meta_ks.delete(key)?;
            db.sync()
        })
        .await
    }

    /// Removes all content for an actor. Returns the number removed.
    #[tracing::instrument(target = TARGET, name = "registry.unregister_all_content", skip(self), fields(%actor_id))]
    pub async fn unregister_all_content(&self, actor_id: Uuid) -> Result<usize> {
        let content_ks = self.inner.content_ks.clone();
        let meta_ks = self.inner.content_meta_ks.clone();
        let db = self.inner.db.clone();

        blocking(move || {
            let keys = content_ks.prefix_keys(actor_id.as_bytes())?;
            let count = keys.len();
            for key in &keys {
                content_ks.delete(*key)?;
                meta_ks.delete(*key)?;
            }
            if count > 0 {
                db.sync()?;
            }
            Ok(count)
        })
        .await
    }

    /// Lists all content IDs for the given actor.
    #[tracing::instrument(target = TARGET, name = "registry.list_content", skip(self), fields(%actor_id))]
    pub async fn list_content(&self, actor_id: Uuid) -> Result<Vec<Uuid>> {
        let ks = self.inner.content_ks.clone();
        blocking(move || ks.resource_ids(actor_id)).await
    }

    /// Lists all content IDs with metadata for the given actor.
    #[tracing::instrument(target = TARGET, name = "registry.list_content_with_metadata", skip(self), fields(%actor_id))]
    pub async fn list_content_with_metadata(
        &self,
        actor_id: Uuid,
    ) -> Result<Vec<(Uuid, ContentMetadata)>> {
        let content_ks = self.inner.content_ks.clone();
        let meta_ks = self.inner.content_meta_ks.clone();

        blocking(move || {
            let ids = content_ks.resource_ids(actor_id)?;
            let mut result = Vec::with_capacity(ids.len());
            for id in ids {
                let key = CompositeKey::new(actor_id, id);
                let meta = match meta_ks.get_bytes(key)? {
                    Some(bytes) => serde_json::from_slice(&bytes)?,
                    None => ContentMetadata::default(),
                };
                result.push((id, meta));
            }
            Ok(result)
        })
        .await
    }

    /// Stores a JSON-serializable value in a keyspace.
    async fn store_json<T: Serialize + Send + 'static>(
        &self,
        ks: &Keyspace,
        key: CompositeKey,
        value: &T,
    ) -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        let ks = ks.clone();
        let db = self.inner.db.clone();
        blocking(move || {
            ks.put(key, &bytes)?;
            db.sync()
        })
        .await
    }

    /// Loads a JSON-deserializable value from a keyspace.
    async fn load_json<T: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        ks: &Keyspace,
        key: CompositeKey,
        kind: &'static str,
    ) -> Result<T> {
        let actor_id = key.actor_id();
        let resource_id = key.resource_id();
        let ks = ks.clone();
        blocking(move || {
            let bytes = ks
                .get_bytes(key)?
                .ok_or_else(|| not_found(kind, actor_id, resource_id))?;
            Ok(serde_json::from_slice(&bytes)?)
        })
        .await
    }

    /// Removes a single entry from a keyspace.
    async fn remove_entry(
        &self,
        ks: &Keyspace,
        key: CompositeKey,
        kind: &'static str,
    ) -> Result<()> {
        let actor_id = key.actor_id();
        let resource_id = key.resource_id();
        let ks = ks.clone();
        let db = self.inner.db.clone();
        blocking(move || {
            if !ks.exists(key)? {
                return Err(not_found(kind, actor_id, resource_id));
            }
            ks.delete(key)?;
            db.sync()
        })
        .await
    }

    /// Removes all entries in a keyspace for an actor. Returns count.
    async fn remove_all_entries(&self, ks: &Keyspace, actor_id: Uuid) -> Result<usize> {
        let ks = ks.clone();
        let db = self.inner.db.clone();
        blocking(move || {
            let keys = ks.prefix_keys(actor_id.as_bytes())?;
            let count = keys.len();
            for key in &keys {
                ks.delete(*key)?;
            }
            if count > 0 {
                db.sync()?;
            }
            Ok(count)
        })
        .await
    }

    /// Lists resource IDs in a keyspace for the given actor.
    async fn list_resource_ids(&self, ks: &Keyspace, actor_id: Uuid) -> Result<Vec<Uuid>> {
        let ks = ks.clone();
        blocking(move || ks.resource_ids(actor_id)).await
    }

    #[tracing::instrument(target = TARGET, name = "registry.register_policy", skip(self, policy), fields(%actor_id))]
    pub async fn register_policy(&self, actor_id: Uuid, policy: Policy<Text>) -> Result<Uuid> {
        let id = policy.id;
        let key = CompositeKey::new(actor_id, id);
        self.store_json(&self.inner.policies_ks, key, &policy)
            .await?;
        tracing::trace!(target: TARGET, %id, "policy registered");
        Ok(id)
    }

    #[tracing::instrument(target = TARGET, name = "registry.read_policy", skip(self), fields(%actor_id, %policy_id))]
    pub async fn read_policy(&self, actor_id: Uuid, policy_id: Uuid) -> Result<Policy<Text>> {
        let key = CompositeKey::new(actor_id, policy_id);
        self.load_json(&self.inner.policies_ks, key, "policy").await
    }

    #[tracing::instrument(target = TARGET, name = "registry.unregister_policy", skip(self), fields(%actor_id, %policy_id))]
    pub async fn unregister_policy(&self, actor_id: Uuid, policy_id: Uuid) -> Result<()> {
        let key = CompositeKey::new(actor_id, policy_id);
        self.remove_entry(&self.inner.policies_ks, key, "policy")
            .await
    }

    #[tracing::instrument(target = TARGET, name = "registry.unregister_all_policies", skip(self), fields(%actor_id))]
    pub async fn unregister_all_policies(&self, actor_id: Uuid) -> Result<usize> {
        self.remove_all_entries(&self.inner.policies_ks, actor_id)
            .await
    }

    #[tracing::instrument(target = TARGET, name = "registry.list_policies", skip(self), fields(%actor_id))]
    pub async fn list_policies(&self, actor_id: Uuid) -> Result<Vec<Uuid>> {
        self.list_resource_ids(&self.inner.policies_ks, actor_id)
            .await
    }

    /// Persist audit trails for a completed pipeline run.
    #[tracing::instrument(target = TARGET, name = "registry.store_audits", skip(self, audits), fields(%actor_id, %run_id))]
    pub async fn store_audits(
        &self,
        actor_id: Uuid,
        run_id: Uuid,
        audits: Vec<AnyAudit>,
    ) -> Result<()> {
        let key = CompositeKey::new(actor_id, run_id);
        let count = audits.len();
        self.store_json(&self.inner.audits_ks, key, &audits).await?;
        tracing::trace!(target: TARGET, count, "audits stored");
        Ok(())
    }

    /// Load persisted audit trails for a pipeline run.
    #[tracing::instrument(target = TARGET, name = "registry.load_audits", skip(self), fields(%actor_id, %run_id))]
    pub async fn load_audits(&self, actor_id: Uuid, run_id: Uuid) -> Result<Vec<AnyAudit>> {
        let key = CompositeKey::new(actor_id, run_id);
        self.load_json(&self.inner.audits_ks, key, "audits").await
    }

    /// Remove persisted audit trails for a pipeline run.
    #[tracing::instrument(target = TARGET, name = "registry.unregister_audits", skip(self), fields(%actor_id, %run_id))]
    pub async fn unregister_audits(&self, actor_id: Uuid, run_id: Uuid) -> Result<()> {
        let key = CompositeKey::new(actor_id, run_id);
        self.remove_entry(&self.inner.audits_ks, key, "audits")
            .await
    }
}

#[cfg(test)]
mod tests {
    use nvisy_codec::content::{Content, ContentData};
    use nvisy_core::ErrorKind;

    use super::*;

    fn temp_registry() -> anyhow::Result<(tempfile::TempDir, Registry)> {
        let temp = tempfile::tempdir()?;
        let registry = Registry::open(temp.path().join("data"))?;
        Ok((temp, registry))
    }

    #[tokio::test]
    async fn register_and_read_content() -> anyhow::Result<()> {
        let (_temp, registry) = temp_registry()?;
        let actor = Uuid::now_v7();
        let handle = registry
            .register_content(actor, Content::new(ContentData::from("Hello, world!")))
            .await?;
        let data = handle.content_data().await?;
        assert_eq!(data.as_str().unwrap(), "Hello, world!");
        Ok(())
    }

    #[tokio::test]
    async fn content_scoped_by_actor() -> anyhow::Result<()> {
        let (_temp, registry) = temp_registry()?;
        let actor_a = Uuid::now_v7();
        let actor_b = Uuid::now_v7();
        let handle = registry
            .register_content(actor_a, Content::new(ContentData::from("actor A only")))
            .await?;
        let id = handle.content_source().as_uuid();
        assert_eq!(
            registry.read_content(actor_b, id).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        registry.read_content(actor_a, id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn list_content_per_actor() -> anyhow::Result<()> {
        let (_temp, registry) = temp_registry()?;
        let actor_a = Uuid::now_v7();
        let actor_b = Uuid::now_v7();
        registry
            .register_content(actor_a, Content::new(ContentData::from("a1")))
            .await?;
        registry
            .register_content(actor_a, Content::new(ContentData::from("a2")))
            .await?;
        registry
            .register_content(actor_b, Content::new(ContentData::from("b1")))
            .await?;
        assert_eq!(registry.list_content(actor_a).await?.len(), 2);
        assert_eq!(registry.list_content(actor_b).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn unregister_content() -> anyhow::Result<()> {
        let (_temp, registry) = temp_registry()?;
        let actor = Uuid::now_v7();
        let content = Content::new(ContentData::from("delete me"));
        let id = content.content_source().as_uuid();
        registry.register_content(actor, content).await?;
        registry.unregister_content(actor, id).await?;
        assert_eq!(
            registry.read_content(actor, id).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        Ok(())
    }

    #[tokio::test]
    async fn unregister_all_content() -> anyhow::Result<()> {
        let (_temp, registry) = temp_registry()?;
        let actor = Uuid::now_v7();
        registry
            .register_content(actor, Content::new(ContentData::from("first")))
            .await?;
        registry
            .register_content(actor, Content::new(ContentData::from("second")))
            .await?;
        assert_eq!(registry.unregister_all_content(actor).await?, 2);
        assert!(registry.list_content(actor).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn data_persists_across_reopen() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("persist");
        let actor = Uuid::now_v7();
        let content = Content::new(ContentData::from("persistent"));
        let id = content.content_source().as_uuid();

        let registry = Registry::open(&path)?;
        registry.register_content(actor, content).await?;
        drop(registry);

        let registry = Registry::open(&path)?;
        let data = registry
            .read_content(actor, id)
            .await?
            .content_data()
            .await?;
        assert_eq!(data.as_str().unwrap(), "persistent");
        Ok(())
    }
}

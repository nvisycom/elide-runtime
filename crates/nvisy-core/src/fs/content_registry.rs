use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::{Error, ErrorKind, Result};
use crate::fs::ContentHandler;
use crate::io::Content;

/// Registry that accepts content, creates temporary directories, and returns
/// handlers that manage the directory lifecycle.
///
/// Each call to [`register`](ContentRegistry::register) creates a subdirectory
/// under the base path, named by the content's [`ContentSource`](crate::path::ContentSource)
/// UUID. The directory is automatically cleaned up when the last
/// [`ContentHandler`] referencing it is dropped.
#[derive(Debug, Clone)]
pub struct ContentRegistry {
    base_dir: PathBuf,
}

impl ContentRegistry {
    /// Creates a new content registry with the specified base directory.
    ///
    /// The directory does not need to exist yet — it is created lazily
    /// when content is first registered.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Registers content and creates a managed temporary directory for it.
    ///
    /// Creates a subdirectory named by the content's `ContentSource` UUID,
    /// writes the content data as `content.bin`, and returns a handler that
    /// deletes the directory when the last reference is dropped.
    pub async fn register(&self, content: Content) -> Result<ContentHandler> {
        let content_source = content.content_source();
        let dir = self.base_dir.join(content_source.to_string());

        tokio::fs::create_dir_all(&dir).await.map_err(|err| {
            Error::new(ErrorKind::InternalError, format!(
                "Failed to create temporary content directory (path: {})", dir.display()
            )).with_source(err)
        })?;

        let data_path = dir.join("content.bin");
        tokio::fs::write(&data_path, content.as_bytes())
            .await
            .map_err(|err| {
                Error::new(ErrorKind::InternalError, format!(
                    "Failed to write content data (path: {})", data_path.display()
                )).with_source(err)
            })?;

        let runtime_handle = tokio::runtime::Handle::current();

        Ok(ContentHandler::new(content_source, dir, runtime_handle))
    }

    /// Returns the base directory path.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Remove a single content directory by UUID.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        let dir = self.base_dir.join(id.to_string());
        tokio::fs::remove_dir_all(&dir).await.map_err(|err| {
            Error::new(
                ErrorKind::InternalError,
                format!("Failed to delete content directory (path: {})", dir.display()),
            )
            .with_source(err)
        })?;
        Ok(())
    }

    /// Remove all content directories under the base dir.
    ///
    /// Returns the number of entries removed.
    pub async fn delete_all(&self) -> Result<usize> {
        let mut entries = tokio::fs::read_dir(&self.base_dir).await.map_err(|err| {
            Error::new(
                ErrorKind::InternalError,
                format!(
                    "Failed to read content directory (path: {})",
                    self.base_dir.display()
                ),
            )
            .with_source(err)
        })?;

        let mut count = 0usize;
        while let Some(entry) = entries.next_entry().await.map_err(|err| {
            Error::new(
                ErrorKind::InternalError,
                format!(
                    "Failed to read content directory entry (path: {})",
                    self.base_dir.display()
                ),
            )
            .with_source(err)
        })? {
            tokio::fs::remove_dir_all(entry.path()).await.map_err(|err| {
                Error::new(
                    ErrorKind::InternalError,
                    format!(
                        "Failed to delete content directory (path: {})",
                        entry.path().display()
                    ),
                )
                .with_source(err)
            })?;
            count += 1;
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use crate::io::{Content, ContentData};

    use super::*;

    #[tokio::test]
    async fn test_register_creates_directory() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::new(temp.path().join("content"));
        let content = Content::new(ContentData::from("Hello, world!"));
        let handler = registry.register(content).await.unwrap();

        assert!(handler.dir().exists());
        assert!(handler.dir().join("content.bin").exists());
    }

    #[tokio::test]
    async fn test_base_dir() {
        let temp = tempfile::TempDir::new().unwrap();
        let base = temp.path().join("content");
        let registry = ContentRegistry::new(&base);
        assert_eq!(registry.base_dir(), base);
    }

    #[tokio::test]
    async fn test_register_multiple() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::new(temp.path().join("content"));

        let h1 = registry
            .register(Content::new(ContentData::from("first")))
            .await
            .unwrap();
        let h2 = registry
            .register(Content::new(ContentData::from("second")))
            .await
            .unwrap();

        assert_ne!(h1.dir(), h2.dir());
        assert!(h1.dir().exists());
        assert!(h2.dir().exists());
    }

    #[tokio::test]
    async fn test_delete() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::new(temp.path().join("content"));
        let content = Content::new(ContentData::from("delete me"));
        let id = content.content_source().as_uuid();
        let handler = registry.register(content).await.unwrap();

        assert!(handler.dir().exists());

        registry.delete(id).await.unwrap();
        assert!(!handler.dir().exists());
    }

    #[tokio::test]
    async fn test_delete_all() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::new(temp.path().join("content"));

        let h1 = registry
            .register(Content::new(ContentData::from("first")))
            .await
            .unwrap();
        let h2 = registry
            .register(Content::new(ContentData::from("second")))
            .await
            .unwrap();

        assert!(h1.dir().exists());
        assert!(h2.dir().exists());

        let deleted = registry.delete_all().await.unwrap();
        assert_eq!(deleted, 2);
        assert!(!h1.dir().exists());
        assert!(!h2.dir().exists());
    }
}

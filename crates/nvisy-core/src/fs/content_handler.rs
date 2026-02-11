use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::path::ContentSource;

/// Inner state cleaned up when the last `ContentHandler` reference is dropped.
struct ContentHandlerInner {
    content_source: ContentSource,
    dir: PathBuf,
    runtime_handle: tokio::runtime::Handle,
}

impl fmt::Debug for ContentHandlerInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContentHandlerInner")
            .field("content_source", &self.content_source)
            .field("dir", &self.dir)
            .finish()
    }
}

impl Drop for ContentHandlerInner {
    fn drop(&mut self) {
        let dir = self.dir.clone();
        let source = self.content_source;

        self.runtime_handle.spawn(async move {
            if let Err(err) = tokio::fs::remove_dir_all(&dir).await {
                tracing::warn!(
                    target: "nvisy_core::fs",
                    content_source = %source,
                    path = %dir.display(),
                    error = %err,
                    "Failed to clean up temporary content directory"
                );
            } else {
                tracing::trace!(
                    target: "nvisy_core::fs",
                    content_source = %source,
                    path = %dir.display(),
                    "Cleaned up temporary content directory"
                );
            }
        });
    }
}

/// Handle to content stored in a managed temporary directory.
///
/// Cloning is cheap — clones share the same underlying directory via `Arc`.
/// When the last clone is dropped, the temporary directory is deleted.
#[derive(Debug, Clone)]
pub struct ContentHandler {
    inner: Arc<ContentHandlerInner>,
}

impl ContentHandler {
    /// Creates a new content handler.
    pub(crate) fn new(
        content_source: ContentSource,
        dir: PathBuf,
        runtime_handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            inner: Arc::new(ContentHandlerInner {
                content_source,
                dir,
                runtime_handle,
            }),
        }
    }

    /// Returns the content source identifier.
    pub fn content_source(&self) -> ContentSource {
        self.inner.content_source
    }

    /// Returns the path to the temporary directory.
    pub fn dir(&self) -> &Path {
        &self.inner.dir
    }
}

#[cfg(test)]
mod tests {
    use crate::fs::ContentRegistry;
    use crate::io::{Content, ContentData};

    #[tokio::test]
    async fn test_handler_has_valid_source() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::new(temp.path().join("content"));
        let content = Content::new(ContentData::from("test data"));
        let handler = registry.register(content).await.unwrap();

        assert!(!handler.content_source().as_uuid().is_nil());
        assert!(handler.dir().exists());
    }

    #[tokio::test]
    async fn test_clone_shares_same_directory() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::new(temp.path().join("content"));
        let content = Content::new(ContentData::from("shared"));
        let handler1 = registry.register(content).await.unwrap();
        let handler2 = handler1.clone();

        assert_eq!(handler1.dir(), handler2.dir());
    }

    #[tokio::test]
    async fn test_directory_cleaned_on_last_drop() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::new(temp.path().join("content"));
        let content = Content::new(ContentData::from("cleanup test"));
        let handler = registry.register(content).await.unwrap();
        let dir = handler.dir().to_path_buf();
        let handler2 = handler.clone();

        assert!(dir.exists());

        drop(handler);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(dir.exists());

        drop(handler2);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(!dir.exists());
    }
}

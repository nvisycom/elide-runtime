//! Content writing trait for async I/O operations
//!
//! This module provides the [`AsyncContentWrite`] trait for writing content data
//! to various async destinations from [`ContentData`] structures.

use std::future::Future;
use std::io;

use tokio::io::{AsyncWrite, AsyncWriteExt};

use super::ContentData;
use crate::fs::ContentMetadata;

/// Trait for writing content to async destinations
///
/// This trait provides methods for writing content data to async destinations
/// with various options for chunking, and verification.
pub trait AsyncContentWrite: AsyncWrite + Unpin + Send {
    /// Write content data to the destination
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails or if there are I/O issues.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::io::{AsyncContentWrite, ContentData};
    /// use nvisy_core::fs::ContentMetadata;
    /// use tokio::fs::File;
    /// use std::io;
    ///
    /// async fn write_file() -> io::Result<ContentMetadata> {
    ///     let mut file = File::create("output.txt").await?;
    ///     let content = ContentData::from("Hello, world!");
    ///     file.write_content(content).await
    /// }
    /// ```
    fn write_content(
        &mut self,
        content_data: ContentData,
    ) -> impl Future<Output = io::Result<ContentMetadata>> + Send
    where
        Self: Sized,
    {
        async move {
            self.write_all(content_data.as_bytes()).await?;
            self.flush().await?;

            let metadata = ContentMetadata::new(content_data.content_source);
            Ok(metadata)
        }
    }

    /// Write content data and return metadata with specified source path
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails or if there are I/O issues.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::io::{AsyncContentWrite, ContentData};
    /// use nvisy_core::fs::ContentMetadata;
    /// use tokio::fs::File;
    /// use std::path::PathBuf;
    /// use std::io;
    ///
    /// async fn write_with_path() -> io::Result<ContentMetadata> {
    ///     let mut file = File::create("output.txt").await?;
    ///     let content = ContentData::from("Hello, world!");
    ///     let path = PathBuf::from("output.txt");
    ///     file.write_content_with_path(content, path).await
    /// }
    /// ```
    fn write_content_with_path(
        &mut self,
        content_data: ContentData,
        path: impl Into<std::path::PathBuf> + Send,
    ) -> impl Future<Output = io::Result<ContentMetadata>> + Send
    where
        Self: Sized,
    {
        async move {
            self.write_all(content_data.as_bytes()).await?;
            self.flush().await?;

            let metadata = ContentMetadata::with_path(content_data.content_source, path);
            Ok(metadata)
        }
    }

    /// Write content data in chunks for better memory efficiency
    ///
    /// This method is useful for writing large content without keeping it
    /// all in memory at once.
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails or if there are I/O issues.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::io::{AsyncContentWrite, ContentData};
    /// use nvisy_core::fs::ContentMetadata;
    /// use tokio::fs::File;
    /// use std::io;
    ///
    /// async fn write_chunked() -> io::Result<ContentMetadata> {
    ///     let mut file = File::create("output.txt").await?;
    ///     let content = ContentData::from(vec![0u8; 1_000_000]); // 1MB
    ///     file.write_content_chunked(content, 8192).await
    /// }
    /// ```
    fn write_content_chunked(
        &mut self,
        content_data: ContentData,
        chunk_size: usize,
    ) -> impl Future<Output = io::Result<ContentMetadata>> + Send
    where
        Self: Sized,
    {
        async move {
            let data = content_data.as_bytes();

            for chunk in data.chunks(chunk_size) {
                self.write_all(chunk).await?;
            }

            self.flush().await?;

            let metadata = ContentMetadata::new(content_data.content_source);
            Ok(metadata)
        }
    }

    /// Write multiple content data items sequentially
    ///
    /// # Errors
    ///
    /// Returns an error if any write operation fails or if there are I/O issues.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::io::{AsyncContentWrite, ContentData};
    /// use nvisy_core::fs::ContentMetadata;
    /// use tokio::fs::File;
    /// use std::io;
    ///
    /// async fn write_multiple() -> io::Result<Vec<ContentMetadata>> {
    ///     let mut file = File::create("output.txt").await?;
    ///     let contents = vec![
    ///         ContentData::from("Hello, "),
    ///         ContentData::from("world!"),
    ///     ];
    ///     file.write_multiple_content(contents).await
    /// }
    /// ```
    fn write_multiple_content(
        &mut self,
        content_data_list: Vec<ContentData>,
    ) -> impl Future<Output = io::Result<Vec<ContentMetadata>>> + Send
    where
        Self: Sized,
    {
        async move {
            let mut metadata_list = Vec::with_capacity(content_data_list.len());

            for content_data in content_data_list {
                self.write_all(content_data.as_bytes()).await?;
                let metadata = ContentMetadata::new(content_data.content_source);
                metadata_list.push(metadata);
            }

            self.flush().await?;
            Ok(metadata_list)
        }
    }

    /// Append content data to the destination without truncating
    ///
    /// This method assumes the destination supports append operations.
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails or if there are I/O issues.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::io::{AsyncContentWrite, ContentData};
    /// use nvisy_core::fs::ContentMetadata;
    /// use tokio::fs::OpenOptions;
    /// use std::io;
    ///
    /// async fn append_content() -> io::Result<ContentMetadata> {
    ///     let mut file = OpenOptions::new()
    ///         .create(true)
    ///         .append(true)
    ///         .open("log.txt")
    ///         .await?;
    ///
    ///     let content = ContentData::from("New log entry\n");
    ///     file.append_content(content).await
    /// }
    /// ```
    fn append_content(
        &mut self,
        content_data: ContentData,
    ) -> impl Future<Output = io::Result<ContentMetadata>> + Send
    where
        Self: Sized,
    {
        async move {
            self.write_all(content_data.as_bytes()).await?;
            self.flush().await?;

            let metadata = ContentMetadata::new(content_data.content_source);
            Ok(metadata)
        }
    }

    /// Write content data with verification
    ///
    /// This method writes the content and then optionally verifies it was
    /// written correctly by checking the expected size.
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails, if there are I/O issues,
    /// or if verification fails.
    fn write_content_verified(
        &mut self,
        content_data: ContentData,
        verify_size: bool,
    ) -> impl Future<Output = io::Result<ContentMetadata>> + Send
    where
        Self: Sized,
    {
        async move {
            let expected_size = content_data.size();
            let data = content_data.as_bytes();

            let bytes_written = self.write(data).await?;
            self.flush().await?;

            if verify_size && bytes_written != expected_size {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    format!(
                        "Expected to write {expected_size} bytes, but only wrote {bytes_written} bytes"
                    ),
                ));
            }

            let metadata = ContentMetadata::new(content_data.content_source);
            Ok(metadata)
        }
    }
}

// Implementations for common types
impl AsyncContentWrite for tokio::fs::File {}
impl AsyncContentWrite for Vec<u8> {}
impl<T: AsyncWrite + Unpin + Send> AsyncContentWrite for Box<T> {}

#[cfg(test)]
mod tests {
    use std::io::Result;

    use super::*;

    #[tokio::test]
    async fn test_write_content() -> Result<()> {
        let mut writer = Vec::<u8>::new();
        let content = ContentData::from("Hello, world!");

        let metadata = writer.write_content(content).await?;
        assert!(!metadata.content_source.as_uuid().is_nil());

        Ok(())
    }

    #[tokio::test]
    async fn test_write_content_with_path() -> Result<()> {
        let mut writer = Vec::<u8>::new();
        let content = ContentData::from("Hello, world!");

        let metadata = writer.write_content_with_path(content, "test.txt").await?;
        assert!(metadata.has_path());
        assert_eq!(metadata.filename(), Some("test.txt"));

        Ok(())
    }

    #[tokio::test]
    async fn test_write_content_chunked() -> Result<()> {
        let mut writer = Vec::<u8>::new();
        let data = vec![42u8; 1000];
        let content = ContentData::from(data.clone());

        let metadata = writer.write_content_chunked(content, 100).await?;
        assert!(!metadata.content_source.as_uuid().is_nil());
        assert_eq!(writer.as_slice(), data.as_slice());

        Ok(())
    }

    #[tokio::test]
    async fn test_write_multiple_content() -> Result<()> {
        let mut writer = Vec::<u8>::new();
        let contents = vec![ContentData::from("Hello, "), ContentData::from("world!")];

        let metadata_list = writer.write_multiple_content(contents).await?;
        assert_eq!(metadata_list.len(), 2);
        assert_eq!(writer.as_slice(), b"Hello, world!");

        Ok(())
    }

    #[tokio::test]
    async fn test_append_content() -> Result<()> {
        let mut writer = Vec::<u8>::new();
        let content = ContentData::from("Hello, world!");

        let metadata = writer.append_content(content).await?;
        assert!(!metadata.content_source.as_uuid().is_nil());
        assert_eq!(writer.as_slice(), b"Hello, world!");

        Ok(())
    }

    #[tokio::test]
    async fn test_write_content_verified() -> Result<()> {
        let mut writer = Vec::<u8>::new();
        let content = ContentData::from("Hello, world!");

        let metadata = writer.write_content_verified(content, true).await?;
        assert!(!metadata.content_source.as_uuid().is_nil());
        assert_eq!(writer.as_slice(), b"Hello, world!");

        Ok(())
    }

    #[tokio::test]
    async fn test_write_empty_content() -> Result<()> {
        let mut writer = Vec::<u8>::new();
        let content = ContentData::from("");

        let metadata = writer.write_content(content).await?;
        assert!(!metadata.content_source.as_uuid().is_nil());
        assert_eq!(writer.as_slice(), b"");

        Ok(())
    }

    #[tokio::test]
    async fn test_write_large_content() -> Result<()> {
        let mut writer = Vec::<u8>::new();
        let data = vec![123u8; 10000];
        let content = ContentData::from(data.clone());

        let metadata = writer.write_content(content).await?;
        assert!(!metadata.content_source.as_uuid().is_nil());
        assert_eq!(writer.as_slice(), data.as_slice());

        Ok(())
    }
}

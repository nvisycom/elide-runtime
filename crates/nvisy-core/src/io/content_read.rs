//! Content reading trait for async I/O operations
//!
//! This module provides the [`AsyncContentRead`] trait for reading content data
//! from various async sources into [`ContentData`] structures.

use std::future::Future;
use std::io;

use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncReadExt};

use super::ContentData;
use crate::path::ContentSource;

/// Trait for reading content from async sources
///
/// This trait provides methods for reading content data from async sources
/// and converting them into [`ContentData`] structures with various options
/// for size limits, and verification.
pub trait AsyncContentRead: AsyncRead + Unpin + Send {
    /// Read all content from the source into a `ContentData` structure
    ///
    /// # Errors
    ///
    /// Returns an error if the read operation fails or if there are I/O issues.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::io::{AsyncContentRead, ContentData};
    /// use tokio::fs::File;
    /// use std::io;
    ///
    /// async fn read_file() -> io::Result<ContentData> {
    ///     let mut file = File::open("example.txt").await?;
    ///     file.read_content().await
    /// }
    /// ```
    fn read_content(&mut self) -> impl Future<Output = io::Result<ContentData>> + Send
    where
        Self: Sized,
    {
        async move {
            let mut buffer = Vec::new();
            self.read_to_end(&mut buffer).await?;

            let content_data = ContentData::new(ContentSource::new(), Bytes::from(buffer));
            Ok(content_data)
        }
    }

    /// Read content with a specified content source
    ///
    /// # Errors
    ///
    /// Returns an error if the read operation fails or if there are I/O issues.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::{io::{AsyncContentRead, ContentData}, path::ContentSource};
    /// use tokio::fs::File;
    /// use std::io;
    ///
    /// async fn read_with_source() -> io::Result<ContentData> {
    ///     let mut file = File::open("example.txt").await?;
    ///     let source = ContentSource::new();
    ///     file.read_content_with_source(source).await
    /// }
    /// ```
    fn read_content_with_source(
        &mut self,
        source: ContentSource,
    ) -> impl Future<Output = io::Result<ContentData>> + Send
    where
        Self: Sized,
    {
        async move {
            let mut buffer = Vec::new();
            self.read_to_end(&mut buffer).await?;

            let content_data = ContentData::new(source, Bytes::from(buffer));
            Ok(content_data)
        }
    }

    /// Read content up to a maximum size limit
    ///
    /// This method prevents reading extremely large files that could cause
    /// memory issues.
    ///
    /// # Errors
    ///
    /// Returns an error if the read operation fails, if there are I/O issues,
    /// or if the content exceeds the maximum size limit.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::io::{AsyncContentRead, ContentData};
    /// use tokio::fs::File;
    /// use std::io;
    ///
    /// async fn read_limited_content() -> io::Result<ContentData> {
    ///     let mut file = File::open("example.txt").await?;
    ///     // Limit to 1MB
    ///     file.read_content_limited(1024 * 1024).await
    /// }
    /// ```
    fn read_content_limited(
        &mut self,
        max_size: usize,
    ) -> impl Future<Output = io::Result<ContentData>> + Send
    where
        Self: Sized,
    {
        async move {
            let mut buffer = Vec::with_capacity(std::cmp::min(max_size, 8192));
            let mut total_read = 0;

            loop {
                let mut temp_buf = vec![0u8; 8192];
                let bytes_read = self.read(&mut temp_buf).await?;

                if bytes_read == 0 {
                    break; // EOF reached
                }

                if total_read + bytes_read > max_size {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Content size exceeds maximum limit of {max_size} bytes"),
                    ));
                }

                buffer.extend_from_slice(&temp_buf[..bytes_read]);
                total_read += bytes_read;
            }

            let content_data = ContentData::new(ContentSource::new(), Bytes::from(buffer));
            Ok(content_data)
        }
    }

    /// Read content in chunks, calling a callback for each chunk
    ///
    /// This is useful for processing large files without loading them
    /// entirely into memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the read operation fails or if the callback
    /// returns an error.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::io::AsyncContentRead;
    /// use tokio::fs::File;
    /// use bytes::Bytes;
    /// use std::io;
    ///
    /// async fn process_chunks() -> io::Result<()> {
    ///     let mut file = File::open("large_file.txt").await?;
    ///
    ///     file.read_content_chunked(8192, |chunk| {
    ///         println!("Processing chunk of {} bytes", chunk.len());
    ///         Ok(())
    ///     }).await
    /// }
    /// ```
    fn read_content_chunked<E>(
        &mut self,
        chunk_size: usize,
        mut callback: impl FnMut(Bytes) -> std::result::Result<(), E> + Send,
    ) -> impl Future<Output = std::result::Result<(), E>> + Send
    where
        Self: Sized,
        E: From<io::Error> + Send,
    {
        async move {
            let mut buffer = vec![0u8; chunk_size];

            loop {
                let bytes_read = self.read(&mut buffer).await?;
                if bytes_read == 0 {
                    break; // EOF reached
                }

                let chunk = Bytes::copy_from_slice(&buffer[..bytes_read]);
                callback(chunk)?;
            }

            Ok(())
        }
    }

    /// Read content with verification
    ///
    /// This method reads the content and optionally verifies it meets
    /// certain criteria.
    ///
    /// # Errors
    ///
    /// Returns an error if the read operation fails, if there are I/O issues,
    /// or if verification fails.
    fn read_content_verified<F>(
        &mut self,
        verify_fn: F,
    ) -> impl Future<Output = io::Result<ContentData>> + Send
    where
        Self: Sized,
        F: FnOnce(&[u8]) -> bool + Send,
    {
        async move {
            let mut buffer = Vec::new();
            self.read_to_end(&mut buffer).await?;

            // Verify with a reference to the buffer data
            if !verify_fn(&buffer) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Content verification failed",
                ));
            }

            // Convert to ContentData after verification
            let content_data = ContentData::new(ContentSource::new(), Bytes::from(buffer));
            Ok(content_data)
        }
    }
}

// Implementations for common types
impl AsyncContentRead for tokio::fs::File {}
impl<T: AsyncRead + Unpin + Send> AsyncContentRead for Box<T> {}

// Test-specific implementations
#[cfg(test)]
impl<T: AsRef<[u8]> + Unpin + Send> AsyncContentRead for std::io::Cursor<T> {}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Result};

    use super::*;

    #[tokio::test]
    async fn test_read_content() -> Result<()> {
        let data = b"Hello, world!";
        let mut cursor = Cursor::new(data);

        let content = cursor.read_content().await.unwrap();
        assert_eq!(content.as_bytes(), data);
        assert_eq!(content.size(), data.len());

        Ok(())
    }

    #[tokio::test]
    async fn test_read_content_with_source() -> Result<()> {
        let data = b"Hello, world!";
        let mut cursor = Cursor::new(data);
        let source = ContentSource::new();

        let content = cursor.read_content_with_source(source).await.unwrap();
        assert_eq!(content.content_source, source);
        assert_eq!(content.as_bytes(), data);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_content_limited() -> Result<()> {
        let data = b"Hello, world!";
        let mut cursor = Cursor::new(data);

        // Should succeed within limit
        let content = cursor.read_content_limited(20).await?;
        assert_eq!(content.as_bytes(), data);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_content_limited_exceeds() -> Result<()> {
        let data = b"Hello, world!";
        let mut cursor = Cursor::new(data);

        // Should fail when exceeding limit
        let result = cursor.read_content_limited(5).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_read_content_chunked() -> Result<()> {
        let data = b"Hello, world!";
        let mut cursor = Cursor::new(data);

        let mut chunks = Vec::new();
        let result = cursor
            .read_content_chunked(5, |chunk| {
                chunks.push(chunk);
                Ok::<(), io::Error>(())
            })
            .await;

        assert!(result.is_ok());
        assert!(!chunks.is_empty());

        // Concatenate chunks and verify they match original data
        let concatenated: Vec<u8> = chunks
            .into_iter()
            .flat_map(|chunk| chunk.to_vec())
            .collect();
        assert_eq!(concatenated, data);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_content_verified() -> Result<()> {
        let data = b"Hello, world!";
        let mut cursor = Cursor::new(data);

        // Should succeed with passing verification
        let content = cursor
            .read_content_verified(|data| !data.is_empty())
            .await?;
        assert_eq!(content.as_bytes(), data);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_content_verified_fails() -> Result<()> {
        let data = b"Hello, world!";
        let mut cursor = Cursor::new(data);

        // Should fail with failing verification
        let result = cursor.read_content_verified(<[u8]>::is_empty).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_read_empty_content() -> Result<()> {
        let data = b"";
        let mut cursor = Cursor::new(data);

        let content = cursor.read_content().await?;
        assert_eq!(content.size(), 0);
        assert!(content.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_read_large_content() -> Result<()> {
        let data = vec![42u8; 10000];
        let mut cursor = Cursor::new(data.clone());

        let content = cursor.read_content().await?;
        assert_eq!(content.as_bytes(), data.as_slice());
        assert_eq!(content.size(), 10000);

        Ok(())
    }
}

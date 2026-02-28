//! Content file handling for filesystem operations
//!
//! This module provides the [`ContentFile`] struct for working with files
//! on the filesystem while maintaining content source tracking and metadata.

use std::io;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt, SeekFrom};

use crate::error::{Error, ErrorKind, Result};
use crate::fs::ContentMetadata;
use crate::io::{AsyncContentRead, AsyncContentWrite, ContentData};
use crate::path::ContentSource;

/// A file wrapper that combines filesystem operations with content tracking
///
/// This struct provides a high-level interface for working with files while
/// maintaining content source identification and metadata throughout the
/// processing pipeline.
#[derive(Debug)]
pub struct ContentFile {
    /// Unique identifier for this content source
    content_source: ContentSource,
    /// The underlying tokio file handle
    file: File,
    /// Path to the file
    path: PathBuf,
}

impl ContentFile {
    /// Create a new `ContentFile` by opening an existing file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or doesn't exist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::fs::ContentFile;
    /// use std::path::Path;
    ///
    /// async fn open_file() -> Result<(), Box<dyn std::error::Error>> {
    ///     let content_file = ContentFile::open("example.txt").await?;
    ///     println!("Opened file with source: {}", content_file.content_source());
    ///     Ok(())
    /// }
    /// ```
    pub async fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = File::open(&path_buf).await?;
        let content_source = ContentSource::new();

        Ok(Self {
            content_source,
            file,
            path: path_buf,
        })
    }

    /// Create a new `ContentFile` with a specific content source
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or read.
    pub async fn open_with_source(
        path: impl AsRef<Path>,
        content_source: ContentSource,
    ) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = File::open(&path_buf).await?;

        Ok(Self {
            content_source,
            file,
            path: path_buf,
        })
    }

    /// Create a new file and return a `ContentFile`
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::fs::ContentFile;
    ///
    /// async fn create_file() -> Result<(), Box<dyn std::error::Error>> {
    ///     let content_file = ContentFile::create("new_file.txt").await?;
    ///     println!("Created file with source: {}", content_file.content_source());
    ///     Ok(())
    /// }
    /// ```
    pub async fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = File::create(&path_buf).await?;
        let content_source = ContentSource::new();

        Ok(Self {
            content_source,
            file,
            path: path_buf,
        })
    }

    /// Create a new file with a specific content source
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created or written to.
    pub async fn create_with_source(
        path: impl AsRef<Path>,
        content_source: ContentSource,
    ) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = File::create(&path_buf).await?;

        Ok(Self {
            content_source,
            file,
            path: path_buf,
        })
    }

    /// Open a file with custom options
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::fs::ContentFile;
    /// use tokio::fs::OpenOptions;
    ///
    /// async fn open_with_options() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut options = OpenOptions::new();
    ///     options.read(true)
    ///         .write(true)
    ///         .create(true);
    ///
    ///     let content_file = ContentFile::open_with_options("data.txt", &options).await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened with the specified options.
    pub async fn open_with_options(
        path: impl AsRef<Path>,
        options: &OpenOptions,
    ) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = options.open(&path_buf).await?;
        let content_source = ContentSource::new();

        Ok(Self {
            content_source,
            file,
            path: path_buf,
        })
    }

    /// Read all content from the file into a `ContentData` structure
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or if an I/O error occurs.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::fs::ContentFile;
    ///
    /// async fn read_content() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut content_file = ContentFile::open("example.txt").await?;
    ///     let content_data = content_file.read_to_content_data().await?;
    ///
    ///     println!("Read {} bytes", content_data.size());
    ///     Ok(())
    /// }
    /// ```
    pub async fn read_to_content_data(&mut self) -> Result<ContentData> {
        let mut buffer = Vec::new();
        self.file.read_to_end(&mut buffer).await?;

        let content_data = ContentData::new(self.content_source, Bytes::from(buffer));

        Ok(content_data)
    }

    /// Read content with size limit to prevent memory issues
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, if an I/O error occurs,
    /// or if the file size exceeds the specified maximum size.
    pub async fn read_to_content_data_limited(&mut self, max_size: usize) -> Result<ContentData> {
        let mut buffer = Vec::new();
        let mut temp_buffer = vec![0u8; 8192];
        let mut total_read = 0;

        loop {
            let bytes_read = self.file.read(&mut temp_buffer).await?;
            if bytes_read == 0 {
                break; // EOF
            }

            if total_read + bytes_read > max_size {
                return Err(Error::new(ErrorKind::Validation, format!(
                    "File size exceeds maximum limit of {max_size} bytes"
                )));
            }

            buffer.extend_from_slice(&temp_buffer[..bytes_read]);
            total_read += bytes_read;
        }

        let content_data = ContentData::new(self.content_source, Bytes::from(buffer));

        Ok(content_data)
    }

    /// Write `ContentData` to the file
    ///
    /// # Errors
    ///
    /// Returns an error if the data cannot be written or if an I/O error occurs.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nvisy_core::fs::ContentFile;
    /// use nvisy_core::io::ContentData;
    ///
    /// async fn write_content() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut content_file = ContentFile::create("output.txt").await?;
    ///     let content_data = ContentData::from("Hello, world!");
    ///
    ///     let metadata = content_file.write_from_content_data(content_data).await?;
    ///     println!("Written to: {:?}", metadata.source_path);
    ///     Ok(())
    /// }
    /// ```
    pub async fn write_from_content_data(
        &mut self,
        content_data: ContentData,
    ) -> Result<ContentMetadata> {
        self.file.write_all(content_data.as_bytes()).await?;
        self.file.flush().await?;

        let metadata = ContentMetadata::with_path(content_data.content_source, self.path.clone());
        Ok(metadata)
    }

    /// Append `ContentData` to the file
    ///
    /// # Errors
    ///
    /// Returns an error if the data cannot be appended or if an I/O error occurs.
    pub async fn append_from_content_data(
        &mut self,
        content_data: ContentData,
    ) -> Result<ContentMetadata> {
        self.file.seek(SeekFrom::End(0)).await?;
        self.file.write_all(content_data.as_bytes()).await?;
        self.file.flush().await?;

        let metadata = ContentMetadata::with_path(content_data.content_source, self.path.clone());
        Ok(metadata)
    }

    /// Write `ContentData` in chunks for better memory efficiency
    ///
    /// # Errors
    ///
    /// Returns an error if the data cannot be written or if an I/O error occurs.
    pub async fn write_from_content_data_chunked(
        &mut self,
        content_data: ContentData,
        chunk_size: usize,
    ) -> Result<ContentMetadata> {
        let data = content_data.as_bytes();

        for chunk in data.chunks(chunk_size) {
            self.file.write_all(chunk).await?;
        }

        self.file.flush().await?;

        let metadata = ContentMetadata::with_path(content_data.content_source, self.path.clone());
        Ok(metadata)
    }

    /// Get content metadata for this file
    pub fn content_metadata(&self) -> ContentMetadata {
        ContentMetadata::with_path(self.content_source, self.path.clone())
    }

    /// Get the file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the content source
    pub fn content_source(&self) -> ContentSource {
        self.content_source
    }

    /// Get the source identifier for this content
    pub fn source(&self) -> ContentSource {
        self.content_source
    }

    /// Get a reference to the underlying file
    pub fn as_file(&self) -> &File {
        &self.file
    }

    /// Get a mutable reference to the underlying file
    pub fn as_file_mut(&mut self) -> &mut File {
        &mut self.file
    }

    /// Convert into the underlying file, consuming the `ContentFile`
    pub fn into_file(self) -> File {
        self.file
    }

    /// Get file size in bytes
    ///
    /// # Errors
    ///
    /// Returns an error if the file metadata cannot be retrieved.
    pub async fn size(&mut self) -> Result<u64> {
        let metadata = self.file.metadata().await?;
        Ok(metadata.len())
    }

    /// Check if the file exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Get the filename
    pub fn filename(&self) -> Option<&str> {
        self.path.file_name().and_then(|name| name.to_str())
    }

    /// Get the file extension
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|ext| ext.to_str())
    }

    /// Sync all data to disk
    ///
    /// # Errors
    ///
    /// Returns an error if the sync operation fails.
    pub async fn sync_all(&mut self) -> Result<()> {
        self.file.sync_all().await?;
        Ok(())
    }

    /// Sync data (but not metadata) to disk
    ///
    /// # Errors
    ///
    /// Returns an error if the sync operation fails.
    pub async fn sync_data(&mut self) -> Result<()> {
        self.file.sync_data().await?;
        Ok(())
    }

    /// Seek to a specific position in the file
    ///
    /// # Errors
    ///
    /// Returns an error if the seek operation fails.
    pub async fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let position = self.file.seek(pos).await?;
        Ok(position)
    }

    /// Get current position in the file
    ///
    /// # Errors
    ///
    /// Returns an error if the current position cannot be determined.
    pub async fn stream_position(&mut self) -> Result<u64> {
        let position = self.file.stream_position().await?;
        Ok(position)
    }
}

// Implement AsyncRead for ContentFile by delegating to the underlying file
impl AsyncRead for ContentFile {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.file).poll_read(cx, buf)
    }
}

// Implement AsyncWrite for ContentFile by delegating to the underlying file
impl AsyncWrite for ContentFile {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        std::pin::Pin::new(&mut self.file).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.file).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.file).poll_shutdown(cx)
    }
}

// Implement AsyncContentRead for ContentFile by delegating to the underlying file
impl AsyncContentRead for ContentFile {
    // Default implementations from the trait will work since File implements AsyncRead
}

// Implement AsyncContentWrite for ContentFile by delegating to the underlying file
impl AsyncContentWrite for ContentFile {
    // Default implementations from the trait will work since File implements AsyncWrite
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[tokio::test]
    async fn test_create_and_open() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create file
        let content_file = ContentFile::create(path).await.unwrap();
        assert_eq!(content_file.path(), path);
        assert!(!content_file.content_source.as_uuid().is_nil());

        // Clean up
        drop(content_file);

        // Open existing file
        let content_file = ContentFile::open(path).await.unwrap();
        assert_eq!(content_file.path(), path);
    }

    #[tokio::test]
    async fn test_write_and_read_content_data() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write content
        let mut content_file = ContentFile::create(path).await.unwrap();
        let content_data = ContentData::from("Hello, world!");
        let metadata = content_file
            .write_from_content_data(content_data)
            .await
            .unwrap();

        assert_eq!(metadata.source_path, Some(path.to_path_buf()));

        // Read content back
        drop(content_file);
        let mut content_file = ContentFile::open(path).await.unwrap();
        let read_content = content_file.read_to_content_data().await.unwrap();

        assert_eq!(read_content.as_string().unwrap(), "Hello, world!");
    }

    #[tokio::test]
    async fn test_file_extension() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut path = temp_file.path().to_path_buf();
        path.set_extension("txt");

        let content_file = ContentFile::create(&path).await.unwrap();
        assert_eq!(content_file.extension(), Some("txt"));
        assert_eq!(
            content_file.filename(),
            path.file_name().and_then(|n| n.to_str())
        );
    }

    #[tokio::test]
    async fn test_write_chunked() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let mut content_file = ContentFile::create(path).await.unwrap();
        let large_data = vec![b'A'; 1000];
        let content_data = ContentData::from(large_data.clone());

        let metadata = content_file
            .write_from_content_data_chunked(content_data, 100)
            .await
            .unwrap();
        assert_eq!(metadata.source_path, Some(path.to_path_buf()));

        // Verify content
        drop(content_file);
        let mut content_file = ContentFile::open(path).await.unwrap();
        let read_content = content_file.read_to_content_data().await.unwrap();

        assert_eq!(read_content.as_bytes(), large_data.as_slice());
    }

    #[tokio::test]
    async fn test_append_content() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write initial content
        let mut content_file = ContentFile::create(path).await.unwrap();
        let initial_content = ContentData::from("Hello, ");
        content_file
            .write_from_content_data(initial_content)
            .await
            .unwrap();

        // Append more content
        let append_content = ContentData::from("world!");
        content_file
            .append_from_content_data(append_content)
            .await
            .unwrap();

        // Verify combined content
        drop(content_file);
        let mut content_file = ContentFile::open(path).await.unwrap();
        let read_content = content_file.read_to_content_data().await.unwrap();

        assert_eq!(read_content.as_string().unwrap(), "Hello, world!");
    }

    #[tokio::test]
    async fn test_read_with_limit() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write content larger than limit
        let mut content_file = ContentFile::create(path).await.unwrap();
        let large_content = ContentData::from(vec![b'X'; 1000]);
        content_file
            .write_from_content_data(large_content)
            .await
            .unwrap();

        drop(content_file);

        // Try to read with small limit
        let mut content_file = ContentFile::open(path).await.unwrap();
        let result = content_file.read_to_content_data_limited(100).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_operations() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let mut content_file = ContentFile::create(path).await.unwrap();

        // Test size (should be 0 for new file)
        let size = content_file.size().await.unwrap();
        assert_eq!(size, 0);

        // Test existence
        assert!(content_file.exists());

        // Write some content
        let content = ContentData::from("Test content");
        content_file.write_from_content_data(content).await.unwrap();

        // Test size after writing
        let size = content_file.size().await.unwrap();
        assert!(size > 0);

        // Test sync operations
        content_file.sync_all().await.unwrap();
        content_file.sync_data().await.unwrap();
    }

    #[tokio::test]
    async fn test_seeking() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let mut content_file = ContentFile::create(path).await.unwrap();
        let content = ContentData::from("0123456789");
        content_file.write_from_content_data(content).await.unwrap();

        // Test seeking
        let pos = content_file.seek(SeekFrom::Start(5)).await.unwrap();
        assert_eq!(pos, 5);

        let current_pos = content_file.stream_position().await.unwrap();
        assert_eq!(current_pos, 5);
    }

    #[tokio::test]
    async fn test_with_specific_source() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let source = ContentSource::new();
        let content_file = ContentFile::create_with_source(path, source).await.unwrap();

        assert_eq!(content_file.content_source, source);

        let metadata = content_file.content_metadata();
        assert_eq!(metadata.content_source, source);
        assert_eq!(metadata.source_path, Some(path.to_path_buf()));
    }
}

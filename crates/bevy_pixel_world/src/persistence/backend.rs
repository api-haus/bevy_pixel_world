//! Async persistence backend traits.
//!
//! Provides [`StorageFile`] and [`StorageFs`] abstractions over random-access
//! file I/O so that `WorldSave` can work on desktop (native files), WASM
//! (OPFS), and iOS without changing its own logic.

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::{fmt, io};

/// Boxed future type used by all trait methods for object safety.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Error type for backend operations.
#[derive(Debug)]
pub enum BackendError {
  /// Standard I/O error.
  Io(io::Error),
  /// File or entry not found.
  NotFound,
  /// Other backend-specific error.
  Other(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for BackendError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Io(e) => write!(f, "I/O error: {e}"),
      Self::NotFound => write!(f, "not found"),
      Self::Other(e) => write!(f, "{e}"),
    }
  }
}

impl Error for BackendError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      Self::Io(e) => Some(e),
      Self::Other(e) => Some(&**e),
      Self::NotFound => None,
    }
  }
}

impl From<io::Error> for BackendError {
  fn from(err: io::Error) -> Self {
    Self::Io(err)
  }
}

impl From<BackendError> for io::Error {
  fn from(err: BackendError) -> Self {
    match err {
      BackendError::Io(e) => e,
      BackendError::NotFound => io::Error::new(io::ErrorKind::NotFound, "not found"),
      BackendError::Other(e) => io::Error::new(io::ErrorKind::Other, e),
    }
  }
}

/// Async random-access file handle.
///
/// All methods take `&self` (not `&mut self`) because positioned I/O
/// (`pread`/`pwrite`) is safe to share. Backends handle internal
/// synchronization as needed.
pub trait StorageFile: Send + Sync {
  /// Reads exactly `buf.len()` bytes starting at `offset`.
  fn read_at(&self, offset: u64, buf: &mut [u8]) -> BoxFuture<'_, Result<(), BackendError>>;

  /// Writes `data` starting at `offset`.
  fn write_at(&self, offset: u64, data: &[u8]) -> BoxFuture<'_, Result<(), BackendError>>;

  /// Returns the current file size in bytes.
  fn len(&self) -> BoxFuture<'_, Result<u64, BackendError>>;

  /// Truncates or extends the file to `size` bytes.
  fn set_len(&self, size: u64) -> BoxFuture<'_, Result<(), BackendError>>;

  /// Flushes all buffered data to durable storage.
  fn sync(&self) -> BoxFuture<'_, Result<(), BackendError>>;
}

/// Async filesystem operations scoped to a directory.
pub trait StorageFs: Send + Sync {
  /// Opens an existing file by name.
  fn open(&self, name: &str) -> BoxFuture<'_, Result<Box<dyn StorageFile>, BackendError>>;

  /// Creates a new file, truncating if it already exists.
  fn create(&self, name: &str) -> BoxFuture<'_, Result<Box<dyn StorageFile>, BackendError>>;

  /// Opens an existing file or creates a new one.
  fn open_or_create(&self, name: &str)
  -> BoxFuture<'_, Result<Box<dyn StorageFile>, BackendError>>;

  /// Returns true if a file with the given name exists.
  fn exists(&self, name: &str) -> BoxFuture<'_, Result<bool, BackendError>>;

  /// Deletes a file by name.
  fn delete(&self, name: &str) -> BoxFuture<'_, Result<(), BackendError>>;

  /// Lists all file names in this directory.
  fn list(&self) -> BoxFuture<'_, Result<Vec<String>, BackendError>>;

  /// Copies a file from one name to another.
  fn copy(&self, from: &str, to: &str) -> BoxFuture<'_, Result<(), BackendError>>;
}

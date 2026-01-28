//! Native filesystem backend using `std::fs::File`.
//!
//! All async methods return immediately-ready futures wrapping synchronous I/O.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use super::backend::{BackendError, BoxFuture, StorageFile, StorageFs};

/// Native file handle wrapping `std::fs::File`.
///
/// Uses a `Mutex<File>` because `seek + read/write` must be atomic on
/// platforms without `pread`/`pwrite`. On Unix this could use positioned I/O
/// directly, but the mutex approach is portable and the contention is
/// negligible for our use case (single-threaded flush).
pub struct NativeFile {
  file: Mutex<fs::File>,
}

impl NativeFile {
  /// Wraps an already-opened `std::fs::File`.
  pub fn new(file: fs::File) -> Self {
    Self {
      file: Mutex::new(file),
    }
  }
}

impl StorageFile for NativeFile {
  fn read_at(&self, offset: u64, buf: &mut [u8]) -> BoxFuture<'_, Result<(), BackendError>> {
    let result = (|| {
      let mut file = self
        .file
        .lock()
        .map_err(|_| BackendError::Io(std::io::Error::other("lock poisoned")))?;
      file.seek(SeekFrom::Start(offset))?;
      file.read_exact(buf)?;
      Ok(())
    })();
    Box::pin(std::future::ready(result))
  }

  fn write_at(&self, offset: u64, data: &[u8]) -> BoxFuture<'_, Result<(), BackendError>> {
    let result = (|| {
      let mut file = self
        .file
        .lock()
        .map_err(|_| BackendError::Io(std::io::Error::other("lock poisoned")))?;
      file.seek(SeekFrom::Start(offset))?;
      file.write_all(data)?;
      Ok(())
    })();
    Box::pin(std::future::ready(result))
  }

  fn len(&self) -> BoxFuture<'_, Result<u64, BackendError>> {
    let result = (|| {
      let file = self
        .file
        .lock()
        .map_err(|_| BackendError::Io(std::io::Error::other("lock poisoned")))?;
      Ok(file.metadata()?.len())
    })();
    Box::pin(std::future::ready(result))
  }

  fn set_len(&self, size: u64) -> BoxFuture<'_, Result<(), BackendError>> {
    let result = (|| {
      let file = self
        .file
        .lock()
        .map_err(|_| BackendError::Io(std::io::Error::other("lock poisoned")))?;
      file.set_len(size)?;
      Ok(())
    })();
    Box::pin(std::future::ready(result))
  }

  fn sync(&self) -> BoxFuture<'_, Result<(), BackendError>> {
    let result = (|| {
      let file = self
        .file
        .lock()
        .map_err(|_| BackendError::Io(std::io::Error::other("lock poisoned")))?;
      file.sync_all()?;
      Ok(())
    })();
    Box::pin(std::future::ready(result))
  }
}

/// Native filesystem backend scoped to a base directory.
pub struct NativeFs {
  base_dir: PathBuf,
}

impl NativeFs {
  /// Creates a new native filesystem backend rooted at `base_dir`.
  ///
  /// Creates the directory if it doesn't exist.
  pub fn new(base_dir: PathBuf) -> std::io::Result<Self> {
    fs::create_dir_all(&base_dir)?;
    Ok(Self { base_dir })
  }

  fn path(&self, name: &str) -> PathBuf {
    self.base_dir.join(name)
  }
}

impl StorageFs for NativeFs {
  fn open(&self, name: &str) -> BoxFuture<'_, Result<Box<dyn StorageFile>, BackendError>> {
    let result = (|| {
      let path = self.path(name);
      if !path.exists() {
        return Err(BackendError::NotFound);
      }
      let file = fs::File::options().read(true).write(true).open(path)?;
      Ok(Box::new(NativeFile::new(file)) as Box<dyn StorageFile>)
    })();
    Box::pin(std::future::ready(result))
  }

  fn create(&self, name: &str) -> BoxFuture<'_, Result<Box<dyn StorageFile>, BackendError>> {
    let result = (|| {
      let path = self.path(name);
      let file = fs::File::create(&path)?;
      // Reopen with read+write since File::create is write-only
      drop(file);
      let file = fs::File::options().read(true).write(true).open(path)?;
      Ok(Box::new(NativeFile::new(file)) as Box<dyn StorageFile>)
    })();
    Box::pin(std::future::ready(result))
  }

  fn open_or_create(
    &self,
    name: &str,
  ) -> BoxFuture<'_, Result<Box<dyn StorageFile>, BackendError>> {
    let result = (|| {
      let path = self.path(name);
      let file = fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
      Ok(Box::new(NativeFile::new(file)) as Box<dyn StorageFile>)
    })();
    Box::pin(std::future::ready(result))
  }

  fn exists(&self, name: &str) -> BoxFuture<'_, Result<bool, BackendError>> {
    let result = Ok(self.path(name).exists());
    Box::pin(std::future::ready(result))
  }

  fn delete(&self, name: &str) -> BoxFuture<'_, Result<(), BackendError>> {
    let result = (|| {
      let path = self.path(name);
      if !path.exists() {
        return Err(BackendError::NotFound);
      }
      fs::remove_file(path)?;
      Ok(())
    })();
    Box::pin(std::future::ready(result))
  }

  fn list(&self) -> BoxFuture<'_, Result<Vec<String>, BackendError>> {
    let result = (|| {
      let mut names = Vec::new();
      for entry in fs::read_dir(&self.base_dir)? {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
          names.push(name.to_string());
        }
      }
      names.sort();
      Ok(names)
    })();
    Box::pin(std::future::ready(result))
  }

  fn copy(&self, from: &str, to: &str) -> BoxFuture<'_, Result<(), BackendError>> {
    let result = (|| {
      let src = self.path(from);
      let dst = self.path(to);
      if !src.exists() {
        return Err(BackendError::NotFound);
      }
      fs::copy(src, dst)?;
      Ok(())
    })();
    Box::pin(std::future::ready(result))
  }
}

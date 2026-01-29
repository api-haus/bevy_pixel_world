//! OPFS (Origin Private File System) storage backend for WASM.
//!
//! Implements [`StorageFile`] and [`StorageFs`] using the web File System
//! Access API. This allows persistent storage in browsers without requiring
//! user interaction.

use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use std::sync::Arc;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
  FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetFileOptions,
  FileSystemReadWriteOptions, FileSystemRemoveOptions, FileSystemSyncAccessHandle,
};

use super::backend::{BackendError, BoxFuture, PersistenceBackend, StorageFile, StorageFs};
use super::{WorldSave, block_on};

/// Converts a JsValue error to BackendError.
fn js_to_backend_error(e: JsValue) -> BackendError {
  let msg = e
    .as_string()
    .or_else(|| js_sys::JSON::stringify(&e).ok().and_then(|s| s.as_string()))
    .unwrap_or_else(|| "Unknown JS error".to_string());
  BackendError::Other(msg.into())
}

/// OPFS file handle using FileSystemSyncAccessHandle.
///
/// The sync access handle provides synchronous read/write operations
/// which is ideal for our use case.
pub struct OpfsFile {
  handle: Rc<RefCell<Option<FileSystemSyncAccessHandle>>>,
}

impl OpfsFile {
  /// Creates a new OpfsFile from a sync access handle.
  fn new(handle: FileSystemSyncAccessHandle) -> Self {
    Self {
      handle: Rc::new(RefCell::new(Some(handle))),
    }
  }
}

impl StorageFile for OpfsFile {
  fn read_at(&self, offset: u64, buf: &mut [u8]) -> BoxFuture<'_, Result<(), BackendError>> {
    let handle = self.handle.clone();
    let len = buf.len();

    let borrowed = handle.borrow();
    let Some(h) = borrowed.as_ref() else {
      return Box::pin(std::future::ready(Err(BackendError::Other(
        "File handle closed".into(),
      ))));
    };

    // Create options with offset
    let options = FileSystemReadWriteOptions::new();
    options.set_at(offset as f64);

    let result = h.read_with_u8_array_and_options(buf, &options);
    drop(borrowed);

    match result {
      Ok(bytes_read) => {
        if bytes_read as usize != len {
          return Box::pin(std::future::ready(Err(BackendError::Io(
            std::io::Error::new(
              std::io::ErrorKind::UnexpectedEof,
              format!("read {} bytes, expected {}", bytes_read, len),
            ),
          ))));
        }
        Box::pin(std::future::ready(Ok(())))
      }
      Err(e) => Box::pin(std::future::ready(Err(js_to_backend_error(e)))),
    }
  }

  fn write_at(&self, offset: u64, data: &[u8]) -> BoxFuture<'_, Result<(), BackendError>> {
    let handle = self.handle.clone();

    let borrowed = handle.borrow();
    let Some(h) = borrowed.as_ref() else {
      return Box::pin(std::future::ready(Err(BackendError::Other(
        "File handle closed".into(),
      ))));
    };

    // Create options with offset
    let options = FileSystemReadWriteOptions::new();
    options.set_at(offset as f64);

    let result = h.write_with_u8_array_and_options(data, &options);
    drop(borrowed);

    match result {
      Ok(_) => Box::pin(std::future::ready(Ok(()))),
      Err(e) => Box::pin(std::future::ready(Err(js_to_backend_error(e)))),
    }
  }

  fn len(&self) -> BoxFuture<'_, Result<u64, BackendError>> {
    let handle = self.handle.clone();

    let borrowed = handle.borrow();
    let Some(h) = borrowed.as_ref() else {
      return Box::pin(std::future::ready(Err(BackendError::Other(
        "File handle closed".into(),
      ))));
    };

    let result = h.get_size();
    drop(borrowed);

    match result {
      Ok(size) => Box::pin(std::future::ready(Ok(size as u64))),
      Err(e) => Box::pin(std::future::ready(Err(js_to_backend_error(e)))),
    }
  }

  fn set_len(&self, size: u64) -> BoxFuture<'_, Result<(), BackendError>> {
    let handle = self.handle.clone();

    let borrowed = handle.borrow();
    let Some(h) = borrowed.as_ref() else {
      return Box::pin(std::future::ready(Err(BackendError::Other(
        "File handle closed".into(),
      ))));
    };

    let result = h.truncate_with_u32(size as u32);
    drop(borrowed);

    match result {
      Ok(_) => Box::pin(std::future::ready(Ok(()))),
      Err(e) => Box::pin(std::future::ready(Err(js_to_backend_error(e)))),
    }
  }

  fn sync(&self) -> BoxFuture<'_, Result<(), BackendError>> {
    let handle = self.handle.clone();

    let borrowed = handle.borrow();
    let Some(h) = borrowed.as_ref() else {
      return Box::pin(std::future::ready(Err(BackendError::Other(
        "File handle closed".into(),
      ))));
    };

    let result = h.flush();
    drop(borrowed);

    match result {
      Ok(_) => Box::pin(std::future::ready(Ok(()))),
      Err(e) => Box::pin(std::future::ready(Err(js_to_backend_error(e)))),
    }
  }
}

impl Drop for OpfsFile {
  fn drop(&mut self) {
    // Close the sync access handle
    if let Some(handle) = self.handle.borrow_mut().take() {
      handle.close();
    }
  }
}

/// OPFS filesystem backend.
///
/// Provides access to the Origin Private File System for persistent storage.
pub struct OpfsFs {
  root: FileSystemDirectoryHandle,
}

impl OpfsFs {
  /// Creates a new OPFS filesystem backend.
  ///
  /// Initializes access to the origin's private file system root.
  pub async fn new() -> Result<Self, BackendError> {
    let window =
      web_sys::window().ok_or_else(|| BackendError::Other("No window object available".into()))?;

    let navigator = window.navigator();
    let storage = navigator.storage();

    let root_promise = storage.get_directory();
    let root = JsFuture::from(root_promise)
      .await
      .map_err(js_to_backend_error)?;

    Ok(Self {
      root: root.unchecked_into(),
    })
  }

  /// Gets a file handle, optionally creating it.
  async fn get_file_handle(
    &self,
    name: &str,
    create: bool,
  ) -> Result<FileSystemFileHandle, BackendError> {
    let options = FileSystemGetFileOptions::new();
    options.set_create(create);

    let promise = self.root.get_file_handle_with_options(name, &options);
    let handle = JsFuture::from(promise).await.map_err(|e| {
      // Check if it's a NotFoundError
      if let Some(err) = e.dyn_ref::<js_sys::Error>() {
        if err.name() == "NotFoundError" {
          return BackendError::NotFound;
        }
      }
      js_to_backend_error(e)
    })?;

    Ok(handle.unchecked_into())
  }

  /// Opens a file handle and creates a sync access handle.
  async fn open_sync_access(
    &self,
    name: &str,
    create: bool,
  ) -> Result<Box<dyn StorageFile>, BackendError> {
    let file_handle = self.get_file_handle(name, create).await?;

    // Create sync access handle for synchronous operations
    let promise = file_handle.create_sync_access_handle();
    let sync_handle = JsFuture::from(promise).await.map_err(js_to_backend_error)?;

    Ok(Box::new(OpfsFile::new(sync_handle.unchecked_into())))
  }
}

impl StorageFs for OpfsFs {
  fn open(&self, name: &str) -> BoxFuture<'_, Result<Box<dyn StorageFile>, BackendError>> {
    let name = name.to_string();
    Box::pin(async move { self.open_sync_access(&name, false).await })
  }

  fn create(&self, name: &str) -> BoxFuture<'_, Result<Box<dyn StorageFile>, BackendError>> {
    let name = name.to_string();
    Box::pin(async move {
      // Delete existing file first to ensure truncation
      let _ = self.delete_impl(&name).await;
      self.open_sync_access(&name, true).await
    })
  }

  fn open_or_create(
    &self,
    name: &str,
  ) -> BoxFuture<'_, Result<Box<dyn StorageFile>, BackendError>> {
    let name = name.to_string();
    Box::pin(async move { self.open_sync_access(&name, true).await })
  }

  fn exists(&self, name: &str) -> BoxFuture<'_, Result<bool, BackendError>> {
    let name = name.to_string();
    Box::pin(async move {
      match self.get_file_handle(&name, false).await {
        Ok(_) => Ok(true),
        Err(BackendError::NotFound) => Ok(false),
        Err(e) => Err(e),
      }
    })
  }

  fn delete(&self, name: &str) -> BoxFuture<'_, Result<(), BackendError>> {
    let name = name.to_string();
    Box::pin(async move { self.delete_impl(&name).await })
  }

  fn list(&self) -> BoxFuture<'_, Result<Vec<String>, BackendError>> {
    Box::pin(async move {
      let mut names = Vec::new();

      // Use entries() iterator
      let entries = self.root.entries();

      loop {
        let promise = entries.next().map_err(js_to_backend_error)?;
        let result = JsFuture::from(promise).await.map_err(js_to_backend_error)?;

        let done = Reflect::get(&result, &"done".into())
          .map_err(js_to_backend_error)?
          .as_bool()
          .unwrap_or(true);

        if done {
          break;
        }

        let value = Reflect::get(&result, &"value".into()).map_err(js_to_backend_error)?;

        // value is [name, handle] array
        let name = Reflect::get_u32(&value, 0)
          .map_err(js_to_backend_error)?
          .as_string();

        if let Some(name) = name {
          names.push(name);
        }
      }

      names.sort();
      Ok(names)
    })
  }

  fn copy(&self, from: &str, to: &str) -> BoxFuture<'_, Result<(), BackendError>> {
    let from = from.to_string();
    let to = to.to_string();
    Box::pin(async move {
      // Read source file
      let src = self.open_sync_access(&from, false).await?;
      let len = src.len().await?;
      let mut data = vec![0u8; len as usize];
      src.read_at(0, &mut data).await?;

      // Write to destination
      let dst = self.open_sync_access(&to, true).await?;
      dst.write_at(0, &data).await?;
      dst.sync().await?;

      Ok(())
    })
  }
}

impl OpfsFs {
  async fn delete_impl(&self, name: &str) -> Result<(), BackendError> {
    let options = FileSystemRemoveOptions::new();

    let promise = self.root.remove_entry_with_options(name, &options);
    JsFuture::from(promise).await.map_err(|e| {
      if let Some(err) = e.dyn_ref::<js_sys::Error>() {
        if err.name() == "NotFoundError" {
          return BackendError::NotFound;
        }
      }
      js_to_backend_error(e)
    })?;

    Ok(())
  }
}

// OPFS operations are not Send on WASM (single-threaded), but we need the trait
// bounds
unsafe impl Send for OpfsFile {}
unsafe impl Sync for OpfsFile {}
unsafe impl Send for OpfsFs {}
unsafe impl Sync for OpfsFs {}

/// WASM persistence backend.
///
/// Wraps `OpfsFs` and provides high-level persistence operations.
/// Must be created from an async context since OPFS initialization is async.
pub struct WasmPersistence {
  fs: Arc<OpfsFs>,
}

impl WasmPersistence {
  /// Creates a new WASM persistence backend.
  ///
  /// Must be called from an async context since OPFS initialization is async.
  pub async fn new() -> Result<Self, BackendError> {
    Ok(Self {
      fs: Arc::new(OpfsFs::new().await?),
    })
  }

  /// Creates a WasmPersistence from an already-initialized OpfsFs.
  pub fn from_fs(fs: Arc<OpfsFs>) -> Self {
    Self { fs }
  }
}

impl PersistenceBackend for WasmPersistence {
  fn list_saves(&self) -> io::Result<Vec<String>> {
    let files = block_on(self.fs.list()).map_err(io::Error::from)?;
    let mut saves: Vec<String> = files
      .into_iter()
      .filter_map(|n| n.strip_suffix(".save").map(String::from))
      .collect();
    saves.sort();
    Ok(saves)
  }

  fn delete_save(&self, name: &str) -> io::Result<()> {
    let file_name = format!("{}.save", name);
    block_on(self.fs.delete(&file_name)).map_err(io::Error::from)
  }

  fn open_or_create_async<'a>(
    &'a self,
    name: &'a str,
    seed: u64,
  ) -> BoxFuture<'a, Result<WorldSave, String>> {
    Box::pin(async move {
      let file_name = format!("{}.save", name);
      WorldSave::open_or_create_async(&*self.fs, &file_name, seed).await
    })
  }

  fn save_copy(&self, save: &mut WorldSave, to_name: &str) -> io::Result<WorldSave> {
    let file_name = format!("{}.save", to_name);
    save.copy_to(&*self.fs, &file_name)
  }

  fn fs(&self) -> &dyn StorageFs {
    &*self.fs
  }

  fn fs_arc(&self) -> Arc<dyn StorageFs> {
    self.fs.clone()
  }
}

// WasmPersistence is safe to send/sync on WASM (single-threaded)
unsafe impl Send for WasmPersistence {}
unsafe impl Sync for WasmPersistence {}

//! WASM I/O worker using Web Worker and postMessage.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use bevy::math::IVec2;
use wasm_bindgen::prelude::*;
use web_sys::{MessageEvent, Worker, WorkerOptions, WorkerType};

use super::{ChunkLoadData, IoCommand, IoResult};

/// WASM I/O dispatcher using a Web Worker.
pub struct WasmIoDispatcher {
  worker: Worker,
  result_queue: Rc<RefCell<VecDeque<IoResult>>>,
  ready: Rc<Cell<bool>>,
  world_seed: Rc<Cell<u64>>,
}

impl WasmIoDispatcher {
  /// Creates a new WASM I/O dispatcher with a Web Worker.
  pub fn new() -> Self {
    let result_queue = Rc::new(RefCell::new(VecDeque::new()));
    let ready = Rc::new(Cell::new(false));
    let world_seed = Rc::new(Cell::new(0u64));

    // Create Web Worker with module type for ES modules
    let mut options = WorkerOptions::new();
    options.set_type(WorkerType::Module);

    let worker =
      Worker::new_with_options("./worker.js", &options).expect("Failed to create Web Worker");

    // Set up message handler
    let queue_clone = Rc::clone(&result_queue);
    let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
      if let Some(result) = parse_worker_message(&event) {
        queue_clone.borrow_mut().push_back(result);
      }
    }) as Box<dyn FnMut(MessageEvent)>);

    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget(); // Keep closure alive

    // Set up error handler
    let onerror = Closure::wrap(Box::new(|event: web_sys::ErrorEvent| {
      web_sys::console::error_1(&format!("Worker error: {:?}", event.message()).into());
    }) as Box<dyn FnMut(web_sys::ErrorEvent)>);

    worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    Self {
      worker,
      result_queue,
      ready,
      world_seed,
    }
  }

  /// Sends a command to the worker.
  pub fn send(&self, cmd: IoCommand) {
    let msg = command_to_js(&cmd);
    if let Err(e) = self.worker.post_message(&msg) {
      web_sys::console::error_1(&format!("Failed to send message to worker: {:?}", e).into());
    }
  }

  /// Tries to receive a result from the worker.
  pub fn try_recv(&self) -> Option<IoResult> {
    self.result_queue.borrow_mut().pop_front()
  }

  /// Returns true if the worker is initialized.
  pub fn is_ready(&self) -> bool {
    self.ready.get()
  }

  /// Sets the ready state.
  pub fn set_ready(&self, ready: bool) {
    self.ready.set(ready);
  }

  /// Returns the world seed if set.
  pub fn world_seed(&self) -> Option<u64> {
    let seed = self.world_seed.get();
    if seed == 0 && !self.is_ready() {
      None
    } else {
      Some(seed)
    }
  }

  /// Sets the world seed.
  pub fn set_world_seed(&self, seed: u64) {
    self.world_seed.set(seed);
  }
}

/// Converts an IoCommand to a JsValue for postMessage.
fn command_to_js(cmd: &IoCommand) -> JsValue {
  let obj = js_sys::Object::new();

  match cmd {
    IoCommand::Initialize { save_name, seed } => {
      js_sys::Reflect::set(&obj, &"type".into(), &"Initialize".into()).unwrap();
      js_sys::Reflect::set(&obj, &"saveName".into(), &save_name.into()).unwrap();
      js_sys::Reflect::set(&obj, &"seed".into(), &JsValue::from_f64(*seed as f64)).unwrap();
    }
    IoCommand::LoadChunk { chunk_pos } => {
      js_sys::Reflect::set(&obj, &"type".into(), &"LoadChunk".into()).unwrap();
      js_sys::Reflect::set(
        &obj,
        &"chunkX".into(),
        &JsValue::from_f64(chunk_pos.x as f64),
      )
      .unwrap();
      js_sys::Reflect::set(
        &obj,
        &"chunkY".into(),
        &JsValue::from_f64(chunk_pos.y as f64),
      )
      .unwrap();
    }
    IoCommand::WriteChunk { chunk_pos, data } => {
      js_sys::Reflect::set(&obj, &"type".into(), &"WriteChunk".into()).unwrap();
      js_sys::Reflect::set(
        &obj,
        &"chunkX".into(),
        &JsValue::from_f64(chunk_pos.x as f64),
      )
      .unwrap();
      js_sys::Reflect::set(
        &obj,
        &"chunkY".into(),
        &JsValue::from_f64(chunk_pos.y as f64),
      )
      .unwrap();
      let arr = js_sys::Uint8Array::from(data.as_slice());
      js_sys::Reflect::set(&obj, &"data".into(), &arr).unwrap();
    }
    IoCommand::SaveBody {
      record_data,
      stable_id,
    } => {
      js_sys::Reflect::set(&obj, &"type".into(), &"SaveBody".into()).unwrap();
      js_sys::Reflect::set(
        &obj,
        &"stableId".into(),
        &JsValue::from_f64(*stable_id as f64),
      )
      .unwrap();
      let arr = js_sys::Uint8Array::from(record_data.as_slice());
      js_sys::Reflect::set(&obj, &"data".into(), &arr).unwrap();
    }
    IoCommand::RemoveBody { stable_id } => {
      js_sys::Reflect::set(&obj, &"type".into(), &"RemoveBody".into()).unwrap();
      js_sys::Reflect::set(
        &obj,
        &"stableId".into(),
        &JsValue::from_f64(*stable_id as f64),
      )
      .unwrap();
    }
    IoCommand::Flush => {
      js_sys::Reflect::set(&obj, &"type".into(), &"Flush".into()).unwrap();
    }
    IoCommand::Shutdown => {
      js_sys::Reflect::set(&obj, &"type".into(), &"Shutdown".into()).unwrap();
    }
  }

  obj.into()
}

/// Parses a worker message into an IoResult.
fn parse_worker_message(event: &MessageEvent) -> Option<IoResult> {
  let data = event.data();
  let obj = data.dyn_ref::<js_sys::Object>()?;

  let type_val = js_sys::Reflect::get(obj, &"type".into()).ok()?;
  let type_str = type_val.as_string()?;

  match type_str.as_str() {
    "Initialized" => {
      let chunk_count = js_sys::Reflect::get(obj, &"chunkCount".into())
        .ok()?
        .as_f64()? as usize;
      let body_count = js_sys::Reflect::get(obj, &"bodyCount".into())
        .ok()?
        .as_f64()? as usize;
      let world_seed = js_sys::Reflect::get(obj, &"worldSeed".into())
        .ok()?
        .as_f64()? as u64;
      Some(IoResult::Initialized {
        chunk_count,
        body_count,
        world_seed,
      })
    }
    "ChunkLoaded" => {
      let chunk_x = js_sys::Reflect::get(obj, &"chunkX".into()).ok()?.as_f64()? as i32;
      let chunk_y = js_sys::Reflect::get(obj, &"chunkY".into()).ok()?.as_f64()? as i32;
      let chunk_pos = IVec2::new(chunk_x, chunk_y);

      let data_val = js_sys::Reflect::get(obj, &"data".into()).ok()?;
      let data = if data_val.is_null() || data_val.is_undefined() {
        None
      } else {
        let storage_type = js_sys::Reflect::get(obj, &"storageType".into())
          .ok()?
          .as_f64()? as u8;
        let seeder_needed = js_sys::Reflect::get(obj, &"seederNeeded".into())
          .ok()?
          .as_bool()?;
        let arr = data_val.dyn_ref::<js_sys::Uint8Array>()?;
        Some(ChunkLoadData {
          storage_type,
          data: arr.to_vec(),
          seeder_needed,
        })
      };

      Some(IoResult::ChunkLoaded { chunk_pos, data })
    }
    "WriteComplete" => {
      let chunk_x = js_sys::Reflect::get(obj, &"chunkX".into()).ok()?.as_f64()? as i32;
      let chunk_y = js_sys::Reflect::get(obj, &"chunkY".into()).ok()?.as_f64()? as i32;
      Some(IoResult::WriteComplete {
        chunk_pos: IVec2::new(chunk_x, chunk_y),
      })
    }
    "BodySaveComplete" => {
      let stable_id = js_sys::Reflect::get(obj, &"stableId".into())
        .ok()?
        .as_f64()? as u64;
      Some(IoResult::BodySaveComplete { stable_id })
    }
    "BodyRemoveComplete" => {
      let stable_id = js_sys::Reflect::get(obj, &"stableId".into())
        .ok()?
        .as_f64()? as u64;
      Some(IoResult::BodyRemoveComplete { stable_id })
    }
    "FlushComplete" => Some(IoResult::FlushComplete),
    "Error" => {
      let message = js_sys::Reflect::get(obj, &"message".into())
        .ok()?
        .as_string()?;
      Some(IoResult::Error { message })
    }
    _ => None,
  }
}

// WASM is single-threaded, but we need the trait bounds
unsafe impl Send for WasmIoDispatcher {}
unsafe impl Sync for WasmIoDispatcher {}

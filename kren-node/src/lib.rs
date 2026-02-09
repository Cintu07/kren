//! Node.js bindings for KREN zero-copy IPC library
//!
//! This module exposes KREN's shared memory IPC functionality to Node.js
//! via napi-rs, allowing JavaScript/TypeScript applications to communicate
//! with other KREN-enabled processes at memory speed.
//!
//! # Example
//!
//! ```javascript
//! const kren = require('kren');
//!
//! // Process A: Create writer
//! const writer = new kren.Writer("my_channel", 4096);
//! writer.write(Buffer.from("Hello from Node!"));
//!
//! // Process B: Connect reader
//! const reader = new kren.Reader("my_channel");
//! const data = reader.read();
//! console.log(data.toString()); // "Hello from Node!"
//! ```

#[macro_use]
extern crate napi_derive;

use napi::{bindgen_prelude::*, JsBuffer, JsBufferValue};
use kren_core::{KrenWriter, KrenReader, KrenError};

/// Convert KREN errors to napi errors
fn to_napi_err(err: KrenError) -> napi::Error {
    match err {
        KrenError::BufferFull { requested, available } => {
            napi::Error::new(
                napi::Status::GenericFailure,
                format!("Buffer full: requested {} bytes, {} available", requested, available),
            )
        }
        KrenError::BufferEmpty => {
            napi::Error::new(napi::Status::GenericFailure, "Buffer empty: no data available")
        }
        KrenError::DataTooLarge { size, max } => {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("Data too large: {} bytes exceeds maximum {}", size, max),
            )
        }
        KrenError::InvalidCapacity(cap) => {
            napi::Error::new(napi::Status::InvalidArg, format!("Invalid capacity: {}", cap))
        }
        KrenError::InvalidMagic => {
            napi::Error::new(napi::Status::GenericFailure, "Invalid KREN buffer (magic number mismatch)")
        }
        KrenError::VersionMismatch { expected, found } => {
            napi::Error::new(
                napi::Status::GenericFailure,
                format!("Version mismatch: expected {}, found {}", expected, found),
            )
        }
        KrenError::ChannelClosed => {
            napi::Error::new(napi::Status::GenericFailure, "Channel closed")
        }
        _ => napi::Error::new(napi::Status::GenericFailure, err.to_string()),
    }
}

/// Writer endpoint for a KREN shared memory channel.
///
/// Creates a new shared memory segment that can be connected to by readers.
/// Only one writer should exist per channel (Single-Producer constraint).
#[napi]
pub struct Writer {
    inner: KrenWriter,
}

#[napi]
impl Writer {
    /// Create a new KREN channel.
    ///
    /// @param name - Unique identifier for the channel
    /// @param capacity - Size of the ring buffer in bytes
    /// @returns A new Writer instance
    /// @throws Error if capacity is invalid or shared memory creation fails
    #[napi(constructor)]
    pub fn new(name: String, capacity: u32) -> napi::Result<Self> {
        let inner = KrenWriter::create(&name, capacity as usize).map_err(to_napi_err)?;
        Ok(Self { inner })
    }

    /// Write data to the channel.
    ///
    /// @param data - Buffer to write
    /// @returns Number of bytes written
    /// @throws Error if buffer is full or data is too large
    #[napi]
    pub fn write(&mut self, data: Buffer) -> napi::Result<u32> {
        let bytes_written = self.inner.write(&data).map_err(to_napi_err)?;
        Ok(bytes_written as u32)
    }

    /// Get the amount of free space in the buffer.
    #[napi(getter)]
    pub fn available(&self) -> u32 {
        self.inner.available_write() as u32
    }

    /// Get the channel name.
    #[napi(getter)]
    pub fn name(&self) -> &str {
        self.inner.name()
    }
}

/// Reader endpoint for a KREN shared memory channel.
///
/// Connects to an existing shared memory segment created by a writer.
/// Only one reader should exist per channel (Single-Consumer constraint).
#[napi]
pub struct Reader {
    inner: KrenReader,
}

#[napi]
impl Reader {
    /// Connect to an existing KREN channel.
    ///
    /// @param name - Name of the channel to connect to
    /// @returns A new Reader instance
    /// @throws Error if channel doesn't exist or connection fails
    #[napi(constructor)]
    pub fn new(name: String) -> napi::Result<Self> {
        let inner = KrenReader::connect(&name).map_err(to_napi_err)?;
        Ok(Self { inner })
    }

    /// Read data from the channel.
    ///
    /// @returns Buffer containing the read data
    /// @throws Error if buffer is empty or channel is closed
    #[napi]
    pub fn read(&mut self, env: Env) -> napi::Result<JsBuffer> {
        let data = self.inner.read().map_err(to_napi_err)?;
        env.create_buffer_with_data(data).map(|b| b.into_raw())
    }

    /// Try to read data without blocking.
    ///
    /// @returns Buffer if data is available, null otherwise
    /// @throws Error if channel error occurs (not including empty buffer)
    #[napi]
    pub fn try_read(&mut self, env: Env) -> napi::Result<Option<JsBuffer>> {
        match self.inner.try_read().map_err(to_napi_err)? {
            Some(data) => {
                let buffer = env.create_buffer_with_data(data)?.into_raw();
                Ok(Some(buffer))
            }
            None => Ok(None),
        }
    }

    /// Check if the writer has closed the channel.
    #[napi(getter)]
    pub fn writer_closed(&self) -> bool {
        self.inner.is_writer_closed()
    }

    /// Get the amount of data available to read.
    #[napi(getter)]
    pub fn available(&self) -> u32 {
        self.inner.available_read() as u32
    }

    /// Get the channel name.
    #[napi(getter)]
    pub fn name(&self) -> &str {
        self.inner.name()
    }
}

/// Get the KREN library version.
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

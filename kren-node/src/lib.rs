#[macro_use]
extern crate napi_derive;

use napi::{bindgen_prelude::*, JsBuffer};
use kren_core::{KrenWriter, KrenReader, KrenError};

fn to_napi_err(err: KrenError) -> napi::Error {
    match err {
        KrenError::BufferFull { requested, available } => {
            napi::Error::new(
                napi::Status::GenericFailure,
                format!("buffer full: requested {} bytes, {} available", requested, available),
            )
        }
        KrenError::BufferEmpty => {
            napi::Error::new(napi::Status::GenericFailure, "buffer empty")
        }
        KrenError::DataTooLarge { size, max } => {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("data too large: {} bytes exceeds max {}", size, max),
            )
        }
        KrenError::InvalidCapacity(cap) => {
            napi::Error::new(napi::Status::InvalidArg, format!("invalid capacity: {}", cap))
        }
        KrenError::InvalidMagic => {
            napi::Error::new(napi::Status::GenericFailure, "invalid magic number")
        }
        KrenError::VersionMismatch { expected, found } => {
            napi::Error::new(
                napi::Status::GenericFailure,
                format!("version mismatch: expected {}, found {}", expected, found),
            )
        }
        KrenError::ChannelClosed => {
            napi::Error::new(napi::Status::GenericFailure, "channel closed")
        }
        _ => napi::Error::new(napi::Status::GenericFailure, err.to_string()),
    }
}

#[napi]
pub struct Writer {
    inner: KrenWriter,
}

#[napi]
impl Writer {
    #[napi(constructor)]
    pub fn new(name: String, capacity: u32) -> napi::Result<Self> {
        let inner = KrenWriter::create(&name, capacity as usize).map_err(to_napi_err)?;
        Ok(Self { inner })
    }

    #[napi]
    pub fn write(&mut self, data: Buffer) -> napi::Result<u32> {
        let bytes_written = self.inner.write(&data).map_err(to_napi_err)?;
        Ok(bytes_written as u32)
    }

    #[napi(getter)]
    pub fn available(&self) -> u32 {
        self.inner.available_write() as u32
    }

    #[napi(getter)]
    pub fn name(&self) -> &str {
        self.inner.name()
    }
}

#[napi]
pub struct Reader {
    inner: KrenReader,
}

#[napi]
impl Reader {
    #[napi(constructor)]
    pub fn new(name: String) -> napi::Result<Self> {
        let inner = KrenReader::connect(&name).map_err(to_napi_err)?;
        Ok(Self { inner })
    }

    #[napi]
    pub fn read(&mut self, env: Env) -> napi::Result<JsBuffer> {
        let data = self.inner.read().map_err(to_napi_err)?;
        env.create_buffer_with_data(data).map(|b| b.into_raw())
    }

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

    #[napi(getter)]
    pub fn writer_closed(&self) -> bool {
        self.inner.is_writer_closed()
    }

    #[napi(getter)]
    pub fn available(&self) -> u32 {
        self.inner.available_read() as u32
    }

    #[napi(getter)]
    pub fn name(&self) -> &str {
        self.inner.name()
    }
}

#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

//! Python bindings for KREN zero-copy IPC library
//!
//! This module exposes KREN's shared memory IPC functionality to Python
//! via PyO3, allowing Python applications to communicate with other
//! KREN-enabled processes (Python, Node.js, or Rust) at memory speed.
//!
//! # Example
//!
//! ```python
//! import kren
//!
//! # Process A: Create writer
//! writer = kren.Writer("my_channel", 4096)
//! writer.write(b"Hello from Python!")
//!
//! # Process B: Connect reader
//! reader = kren.Reader("my_channel")
//! data = reader.read()
//! print(data)  # b"Hello from Python!"
//! ```

use pyo3::prelude::*;
use pyo3::exceptions::{PyIOError, PyValueError, PyRuntimeError};
use pyo3::types::PyBytes;
use kren_core::{KrenWriter, KrenReader, KrenError};

/// Convert KREN errors to Python exceptions
fn to_py_err(err: KrenError) -> PyErr {
    match err {
        KrenError::BufferFull { requested, available } => {
            PyIOError::new_err(format!(
                "Buffer full: requested {} bytes, {} available",
                requested, available
            ))
        }
        KrenError::BufferEmpty => {
            PyIOError::new_err("Buffer empty: no data available")
        }
        KrenError::DataTooLarge { size, max } => {
            PyValueError::new_err(format!(
                "Data too large: {} bytes exceeds maximum {}",
                size, max
            ))
        }
        KrenError::InvalidCapacity(cap) => {
            PyValueError::new_err(format!("Invalid capacity: {}", cap))
        }
        KrenError::InvalidMagic => {
            PyRuntimeError::new_err("Invalid KREN buffer (magic number mismatch)")
        }
        KrenError::VersionMismatch { expected, found } => {
            PyRuntimeError::new_err(format!(
                "Version mismatch: expected {}, found {}",
                expected, found
            ))
        }
        KrenError::ChannelClosed => {
            PyIOError::new_err("Channel closed")
        }
        _ => PyRuntimeError::new_err(err.to_string()),
    }
}

/// Writer endpoint for a KREN shared memory channel.
///
/// Creates a new shared memory segment that can be connected to by readers.
/// Only one writer should exist per channel (Single-Producer constraint).
#[pyclass(name = "Writer")]
pub struct PyKrenWriter {
    inner: KrenWriter,
}

#[pymethods]
impl PyKrenWriter {
    /// Create a new KREN channel.
    ///
    /// Args:
    ///     name: Unique identifier for the channel
    ///     capacity: Size of the ring buffer in bytes
    ///
    /// Returns:
    ///     A new Writer instance
    ///
    /// Raises:
    ///     ValueError: If capacity is invalid
    ///     RuntimeError: If shared memory creation fails
    #[new]
    fn new(name: &str, capacity: usize) -> PyResult<Self> {
        let inner = KrenWriter::create(name, capacity).map_err(to_py_err)?;
        Ok(Self { inner })
    }

    /// Write data to the channel.
    ///
    /// Args:
    ///     data: Bytes to write
    ///
    /// Returns:
    ///     Number of bytes written
    ///
    /// Raises:
    ///     IOError: If buffer is full
    ///     ValueError: If data is too large for the buffer
    fn write(&mut self, data: &[u8]) -> PyResult<usize> {
        self.inner.write(data).map_err(to_py_err)
    }

    /// Get the amount of free space in the buffer.
    ///
    /// Returns:
    ///     Number of bytes available for writing
    #[getter]
    fn available(&self) -> usize {
        self.inner.available_write()
    }

    /// Get the channel name.
    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }
}

/// Reader endpoint for a KREN shared memory channel.
///
/// Connects to an existing shared memory segment created by a writer.
/// Only one reader should exist per channel (Single-Consumer constraint).
#[pyclass(name = "Reader")]
pub struct PyKrenReader {
    inner: KrenReader,
}

#[pymethods]
impl PyKrenReader {
    /// Connect to an existing KREN channel.
    ///
    /// Args:
    ///     name: Name of the channel to connect to
    ///
    /// Returns:
    ///     A new Reader instance
    ///
    /// Raises:
    ///     RuntimeError: If channel doesn't exist or connection fails
    #[new]
    fn new(name: &str) -> PyResult<Self> {
        let inner = KrenReader::connect(name).map_err(to_py_err)?;
        Ok(Self { inner })
    }

    /// Read data from the channel.
    ///
    /// Returns:
    ///     Bytes read from the channel
    ///
    /// Raises:
    ///     IOError: If buffer is empty or channel is closed
    fn read<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let data = self.inner.read().map_err(to_py_err)?;
        Ok(PyBytes::new_bound(py, &data))
    }

    /// Try to read data without blocking.
    ///
    /// Returns:
    ///     Bytes if data is available, None otherwise
    ///
    /// Raises:
    ///     RuntimeError: If channel error occurs (not including empty buffer)
    fn try_read<'py>(&mut self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyBytes>>> {
        match self.inner.try_read().map_err(to_py_err)? {
            Some(data) => Ok(Some(PyBytes::new_bound(py, &data))),
            None => Ok(None),
        }
    }

    /// Check if the writer has closed the channel.
    #[getter]
    fn writer_closed(&self) -> bool {
        self.inner.is_writer_closed()
    }

    /// Get the amount of data available to read.
    #[getter]
    fn available(&self) -> usize {
        self.inner.available_read()
    }

    /// Get the channel name.
    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }
}

/// KREN - Zero-Copy Shared Memory IPC
///
/// High-performance inter-process communication using shared memory
/// with lock-free ring buffers.
///
/// Example:
///     >>> import kren
///     >>> writer = kren.Writer("channel", 4096)
///     >>> writer.write(b"Hello!")
///     6
///     >>> reader = kren.Reader("channel")
///     >>> reader.read()
///     b'Hello!'
#[pymodule]
fn kren(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<PyKrenWriter>()?;
    m.add_class::<PyKrenReader>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writer_reader_roundtrip() {
        pyo3::prepare_freethreaded_python();
        
        Python::with_gil(|_py| {
            let mut writer = PyKrenWriter::new("py_test_roundtrip", 1024).unwrap();
            let reader = PyKrenReader::new("py_test_roundtrip").unwrap();
            
            let written = writer.write(b"Hello from test!").unwrap();
            assert_eq!(written, 16);
        });
    }
}

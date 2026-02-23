use pyo3::prelude::*;
use pyo3::exceptions::{PyIOError, PyValueError, PyRuntimeError};
use pyo3::types::PyBytes;
use kren_core::{KrenWriter, KrenReader, KrenError};

fn to_py_err(err: KrenError) -> PyErr {
    match err {
        KrenError::BufferFull { requested, available } => {
            PyIOError::new_err(format!(
                "buffer full: requested {} bytes, {} available",
                requested, available
            ))
        }
        KrenError::BufferEmpty => {
            PyIOError::new_err("buffer empty")
        }
        KrenError::DataTooLarge { size, max } => {
            PyValueError::new_err(format!(
                "data too large: {} bytes exceeds max {}",
                size, max
            ))
        }
        KrenError::InvalidCapacity(cap) => {
            PyValueError::new_err(format!("invalid capacity: {}", cap))
        }
        KrenError::InvalidMagic => {
            PyRuntimeError::new_err("invalid magic number - memory corrupted")
        }
        KrenError::VersionMismatch { expected, found } => {
            PyRuntimeError::new_err(format!(
                "version mismatch: expected {}, found {}",
                expected, found
            ))
        }
        KrenError::ChannelClosed => {
            PyIOError::new_err("channel closed")
        }
        _ => PyRuntimeError::new_err(err.to_string()),
    }
}

#[pyclass(name = "Writer")]
pub struct PyKrenWriter {
    inner: KrenWriter,
}

#[pymethods]
impl PyKrenWriter {
    #[new]
    fn new(name: &str, capacity: usize) -> PyResult<Self> {
        let inner = KrenWriter::create(name, capacity).map_err(to_py_err)?;
        Ok(Self { inner })
    }

    fn write(&mut self, data: &[u8]) -> PyResult<usize> {
        self.inner.write(data).map_err(to_py_err)
    }

    #[getter]
    fn available(&self) -> usize {
        self.inner.available_write()
    }

    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }
}

#[pyclass(name = "Reader")]
pub struct PyKrenReader {
    inner: KrenReader,
}

#[pymethods]
impl PyKrenReader {
    #[new]
    fn new(name: &str) -> PyResult<Self> {
        let inner = KrenReader::connect(name).map_err(to_py_err)?;
        Ok(Self { inner })
    }

    fn read<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let data = self.inner.read().map_err(to_py_err)?;
        Ok(PyBytes::new_bound(py, &data))
    }

    fn try_read<'py>(&mut self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyBytes>>> {
        match self.inner.try_read().map_err(to_py_err)? {
            Some(data) => Ok(Some(PyBytes::new_bound(py, &data))),
            None => Ok(None),
        }
    }

    #[getter]
    fn writer_closed(&self) -> bool {
        self.inner.is_writer_closed()
    }

    #[getter]
    fn available(&self) -> usize {
        self.inner.available_read()
    }

    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }
}

#[pymodule]
fn kren(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<PyKrenWriter>()?;
    m.add_class::<PyKrenReader>()?;
    Ok(())
}

//! KREN - Zero-Copy Shared Memory IPC Library
//!
//! KREN provides high-performance inter-process communication using shared memory
//! with lock-free ring buffers. It bypasses standard OS networking and serialization
//! to achieve nanosecond-level latency.
//!
//! # Quick Start
//!
//! ```no_run
//! use kren_core::{KrenWriter, KrenReader};
//!
//! // Process A: Create writer
//! let mut writer = KrenWriter::create("my_channel", 4096).unwrap();
//! writer.write(b"Hello from Process A!").unwrap();
//!
//! // Process B: Connect reader
//! let mut reader = KrenReader::connect("my_channel").unwrap();
//! let data = reader.read().unwrap();
//! ```

pub mod error;
pub mod header;
pub mod platform;
pub mod buffer;

pub use error::{KrenError, Result};
pub use header::{SharedHeader, KREN_MAGIC, KREN_VERSION};
pub use buffer::RingBuffer;

#[cfg(windows)]
pub use platform::WindowsSharedMemory;

#[cfg(unix)]
pub use platform::UnixSharedMemory;

pub use platform::PlatformShm;
use platform::SharedMemory;

/// Writer endpoint for a KREN channel
/// 
/// The writer creates the shared memory segment and writes data to the ring buffer.
/// Only one writer should exist per channel (Single-Producer constraint).
pub struct KrenWriter {
    shm: PlatformShm,
    buffer: RingBuffer,
}

impl KrenWriter {
    /// Create a new KREN channel with the given name and capacity
    /// 
    /// # Arguments
    /// * `name` - Unique identifier for the channel
    /// * `capacity` - Size of the ring buffer in bytes
    /// 
    /// # Returns
    /// A new `KrenWriter` instance
    pub fn create(name: &str, capacity: usize) -> Result<Self> {
        if capacity == 0 || capacity > u32::MAX as usize {
            return Err(KrenError::InvalidCapacity(capacity));
        }

        let total_size = SharedHeader::SIZE + capacity;
        let shm = PlatformShm::create(name, total_size)?;

        // Initialize the header
        let header = unsafe { SharedHeader::init(shm.as_ptr(), capacity as u32) };
        
        // Create the ring buffer view
        let data_ptr = unsafe { shm.as_ptr().add(SharedHeader::SIZE) };
        let buffer = RingBuffer::new(header, data_ptr, capacity);

        Ok(Self { shm, buffer })
    }

    /// Write data to the channel
    /// 
    /// # Arguments
    /// * `data` - Bytes to write
    /// 
    /// # Returns
    /// Number of bytes written, or error if buffer is full
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        self.buffer.write(data)
    }

    /// Get the amount of free space in the buffer
    pub fn available_write(&self) -> usize {
        self.buffer.available_write()
    }

    /// Get the channel name
    pub fn name(&self) -> &str {
        self.shm.name()
    }
}

impl Drop for KrenWriter {
    fn drop(&mut self) {
        // Signal that writer is closing
        let header = unsafe { SharedHeader::from_ptr(self.shm.as_ptr()) };
        header.set_flags(header::ChannelFlags::WriterClosed);
    }
}

/// Reader endpoint for a KREN channel
/// 
/// The reader connects to an existing shared memory segment and reads data.
/// Only one reader should exist per channel (Single-Consumer constraint).
pub struct KrenReader {
    shm: PlatformShm,
    buffer: RingBuffer,
}

impl KrenReader {
    /// Connect to an existing KREN channel
    /// 
    /// # Arguments
    /// * `name` - Name of the channel to connect to
    /// 
    /// # Returns
    /// A new `KrenReader` instance
    pub fn connect(name: &str) -> Result<Self> {
        let shm = PlatformShm::open(name)?;
        
        let header = unsafe { SharedHeader::from_ptr(shm.as_ptr()) };
        header.validate()?;

        let capacity = header.capacity as usize;
        let data_ptr = unsafe { shm.as_ptr().add(SharedHeader::SIZE) };
        let buffer = RingBuffer::new(header, data_ptr, capacity);

        Ok(Self { shm, buffer })
    }

    /// Read data from the channel
    /// 
    /// Blocks conceptually until data is available (in practice, returns immediately).
    /// Use `try_read` for non-blocking reads.
    /// 
    /// # Returns
    /// The data read, or error if buffer is empty or channel closed
    pub fn read(&mut self) -> Result<Vec<u8>> {
        self.buffer.read()
    }

    /// Try to read data without blocking
    /// 
    /// # Returns
    /// * `Ok(Some(data))` - Data was available
    /// * `Ok(None)` - No data available (buffer empty)
    /// * `Err(...)` - Channel error
    pub fn try_read(&mut self) -> Result<Option<Vec<u8>>> {
        match self.buffer.read() {
            Ok(data) => Ok(Some(data)),
            Err(KrenError::BufferEmpty) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Check if the writer has closed the channel
    pub fn is_writer_closed(&self) -> bool {
        let header = unsafe { SharedHeader::from_ptr(self.shm.as_ptr()) };
        matches!(header.get_flags(), header::ChannelFlags::WriterClosed | header::ChannelFlags::Closed)
    }

    /// Get the amount of data available to read
    pub fn available_read(&self) -> usize {
        self.buffer.available_read()
    }

    /// Get the channel name
    pub fn name(&self) -> &str {
        self.shm.name()
    }
}

impl Drop for KrenReader {
    fn drop(&mut self) {
        // Signal that reader is closing
        let header = unsafe { SharedHeader::from_ptr(self.shm.as_ptr()) };
        let current = header.get_flags();
        let new_flags = match current {
            header::ChannelFlags::Active => header::ChannelFlags::ReaderClosed,
            header::ChannelFlags::WriterClosed => header::ChannelFlags::Closed,
            _ => header::ChannelFlags::Closed,
        };
        header.set_flags(new_flags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writer_reader_basic() {
        let name = "test_wr_basic";
        let mut writer = KrenWriter::create(name, 1024).expect("Failed to create writer");
        let mut reader = KrenReader::connect(name).expect("Failed to connect reader");

        // Write some data
        let data = b"Hello, KREN!";
        let written = writer.write(data).expect("Failed to write");
        assert_eq!(written, data.len());

        // Read it back
        let received = reader.read().expect("Failed to read");
        assert_eq!(received, data);
    }

    #[test]
    fn test_multiple_writes_reads() {
        let name = "test_multi_wr";
        let mut writer = KrenWriter::create(name, 1024).expect("Failed to create");
        let mut reader = KrenReader::connect(name).expect("Failed to connect");

        for i in 0..10 {
            let msg = format!("Message {}", i);
            writer.write(msg.as_bytes()).expect("Write failed");
            let received = reader.read().expect("Read failed");
            assert_eq!(received, msg.as_bytes());
        }
    }

    #[test]
    fn test_try_read_empty() {
        let name = "test_try_read";
        let _writer = KrenWriter::create(name, 1024).expect("Failed to create");
        let mut reader = KrenReader::connect(name).expect("Failed to connect");

        // Buffer should be empty
        assert!(reader.try_read().expect("try_read failed").is_none());
    }

    #[test]
    fn test_writer_closed_flag() {
        let name = "test_writer_closed";
        let reader;
        
        {
            let writer = KrenWriter::create(name, 1024).expect("Failed to create");
            reader = KrenReader::connect(name).expect("Failed to connect");
            assert!(!reader.is_writer_closed());
            drop(writer); // Writer drops here
        }

        assert!(reader.is_writer_closed());
    }
}

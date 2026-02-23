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

pub struct KrenWriter {
    shm: PlatformShm,
    buffer: RingBuffer,
}

impl KrenWriter {
    pub fn create(name: &str, capacity: usize) -> Result<Self> {
        if capacity == 0 || capacity > u32::MAX as usize {
            return Err(KrenError::InvalidCapacity(capacity));
        }

        let total_size = SharedHeader::SIZE + capacity;
        let shm = PlatformShm::create(name, total_size)?;
        let header = unsafe { SharedHeader::init(shm.as_ptr(), capacity as u32) };
        let data_ptr = unsafe { shm.as_ptr().add(SharedHeader::SIZE) };
        let buffer = RingBuffer::new(header, data_ptr, capacity);

        Ok(Self { shm, buffer })
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        self.buffer.write(data)
    }

    pub fn available_write(&self) -> usize {
        self.buffer.available_write()
    }

    pub fn name(&self) -> &str {
        self.shm.name()
    }
}

impl Drop for KrenWriter {
    fn drop(&mut self) {
        let header = unsafe { SharedHeader::from_ptr(self.shm.as_ptr()) };
        header.set_flags(header::ChannelFlags::WriterClosed);
    }
}

pub struct KrenReader {
    shm: PlatformShm,
    buffer: RingBuffer,
}

impl KrenReader {
    pub fn connect(name: &str) -> Result<Self> {
        let shm = PlatformShm::open(name)?;
        let header = unsafe { SharedHeader::from_ptr(shm.as_ptr()) };
        header.validate()?;

        let capacity = header.capacity as usize;
        let data_ptr = unsafe { shm.as_ptr().add(SharedHeader::SIZE) };
        let buffer = RingBuffer::new(header, data_ptr, capacity);

        Ok(Self { shm, buffer })
    }

    pub fn read(&mut self) -> Result<Vec<u8>> {
        self.buffer.read()
    }

    pub fn try_read(&mut self) -> Result<Option<Vec<u8>>> {
        match self.buffer.read() {
            Ok(data) => Ok(Some(data)),
            Err(KrenError::BufferEmpty) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn is_writer_closed(&self) -> bool {
        let header = unsafe { SharedHeader::from_ptr(self.shm.as_ptr()) };
        matches!(header.get_flags(), header::ChannelFlags::WriterClosed | header::ChannelFlags::Closed)
    }

    pub fn available_read(&self) -> usize {
        self.buffer.available_read()
    }

    pub fn name(&self) -> &str {
        self.shm.name()
    }
}

impl Drop for KrenReader {
    fn drop(&mut self) {
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
        let mut writer = KrenWriter::create("test_wr_basic", 1024).unwrap();
        let mut reader = KrenReader::connect("test_wr_basic").unwrap();

        let data = b"Hello, KREN!";
        let written = writer.write(data).unwrap();
        assert_eq!(written, data.len());

        let received = reader.read().unwrap();
        assert_eq!(received, data);
    }

    #[test]
    fn test_multiple_writes_reads() {
        let mut writer = KrenWriter::create("test_multi_wr", 1024).unwrap();
        let mut reader = KrenReader::connect("test_multi_wr").unwrap();

        for i in 0..10 {
            let msg = format!("Message {}", i);
            writer.write(msg.as_bytes()).unwrap();
            let received = reader.read().unwrap();
            assert_eq!(received, msg.as_bytes());
        }
    }

    #[test]
    fn test_try_read_empty() {
        let _writer = KrenWriter::create("test_try_read", 1024).unwrap();
        let mut reader = KrenReader::connect("test_try_read").unwrap();
        assert!(reader.try_read().unwrap().is_none());
    }

    #[test]
    fn test_writer_closed_flag() {
        let reader;
        {
            let writer = KrenWriter::create("test_writer_closed", 1024).unwrap();
            reader = KrenReader::connect("test_writer_closed").unwrap();
            assert!(!reader.is_writer_closed());
            drop(writer);
        }
        assert!(reader.is_writer_closed());
    }
}

use crate::error::{KrenError, Result};
use crate::header::SharedHeader;
use std::ptr;

const MSG_HEADER_SIZE: usize = 4;

pub struct RingBuffer {
    header: *mut SharedHeader,
    data: *mut u8,
    capacity: usize,
}

unsafe impl Send for RingBuffer {}
unsafe impl Sync for RingBuffer {}

impl RingBuffer {
    pub fn new(header: &mut SharedHeader, data: *mut u8, capacity: usize) -> Self {
        Self {
            header: header as *mut SharedHeader,
            data,
            capacity,
        }
    }

    #[inline]
    fn header(&self) -> &SharedHeader {
        unsafe { &*self.header }
    }

    pub fn write(&self, data: &[u8]) -> Result<usize> {
        let len = data.len();
        let total_needed = MSG_HEADER_SIZE + len;

        if total_needed > self.capacity {
            return Err(KrenError::DataTooLarge {
                size: len,
                max: self.capacity - MSG_HEADER_SIZE,
            });
        }

        let available = self.available_write();
        if total_needed > available {
            return Err(KrenError::BufferFull {
                requested: len,
                available: available.saturating_sub(MSG_HEADER_SIZE),
            });
        }

        let head = self.header().head() as usize;

        let len_bytes = (len as u32).to_le_bytes();
        self.write_at(head, &len_bytes);
        self.write_at((head + MSG_HEADER_SIZE) % self.capacity, data);

        let new_head = (head + total_needed) % self.capacity;
        self.header().set_head(new_head as u32);

        Ok(len)
    }

    pub fn read(&self) -> Result<Vec<u8>> {
        if self.available_read() == 0 {
            return Err(KrenError::BufferEmpty);
        }

        let tail = self.header().tail() as usize;

        let mut len_bytes = [0u8; 4];
        self.read_at(tail, &mut len_bytes);
        let len = u32::from_le_bytes(len_bytes) as usize;

        let mut data = vec![0u8; len];
        self.read_at((tail + MSG_HEADER_SIZE) % self.capacity, &mut data);

        let new_tail = (tail + MSG_HEADER_SIZE + len) % self.capacity;
        self.header().set_tail(new_tail as u32);

        Ok(data)
    }

    pub fn available_write(&self) -> usize {
        let head = self.header().head() as usize;
        let tail = self.header().tail() as usize;

        if head >= tail {
            self.capacity - (head - tail) - 1
        } else {
            tail - head - 1
        }
    }

    pub fn available_read(&self) -> usize {
        let head = self.header().head() as usize;
        let tail = self.header().tail() as usize;

        if head >= tail {
            head - tail
        } else {
            self.capacity - tail + head
        }
    }

    fn write_at(&self, pos: usize, data: &[u8]) {
        let first_chunk = std::cmp::min(data.len(), self.capacity - pos);

        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), self.data.add(pos), first_chunk);

            if first_chunk < data.len() {
                ptr::copy_nonoverlapping(
                    data.as_ptr().add(first_chunk),
                    self.data,
                    data.len() - first_chunk,
                );
            }
        }
    }

    fn read_at(&self, pos: usize, buf: &mut [u8]) {
        let first_chunk = std::cmp::min(buf.len(), self.capacity - pos);

        unsafe {
            ptr::copy_nonoverlapping(self.data.add(pos), buf.as_mut_ptr(), first_chunk);

            if first_chunk < buf.len() {
                ptr::copy_nonoverlapping(
                    self.data,
                    buf.as_mut_ptr().add(first_chunk),
                    buf.len() - first_chunk,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_buffer(capacity: usize) -> (Vec<u8>, *mut u8) {
        let total = SharedHeader::SIZE + capacity;
        let mut buffer = vec![0u8; total];
        let ptr = buffer.as_mut_ptr();
        (buffer, ptr)
    }

    #[test]
    fn test_write_read_single() {
        let capacity = 256;
        let (_buffer, ptr) = create_test_buffer(capacity);
        let header = unsafe { SharedHeader::init(ptr, capacity as u32) };
        let data_ptr = unsafe { ptr.add(SharedHeader::SIZE) };
        let ring = RingBuffer::new(header, data_ptr, capacity);

        let msg = b"Hello, World!";
        let written = ring.write(msg).unwrap();
        assert_eq!(written, msg.len());

        let received = ring.read().unwrap();
        assert_eq!(received, msg);
    }

    #[test]
    fn test_buffer_full() {
        let capacity = 64;
        let (_buffer, ptr) = create_test_buffer(capacity);
        let header = unsafe { SharedHeader::init(ptr, capacity as u32) };
        let data_ptr = unsafe { ptr.add(SharedHeader::SIZE) };
        let ring = RingBuffer::new(header, data_ptr, capacity);

        let big_data = vec![0u8; capacity];
        assert!(matches!(ring.write(&big_data), Err(KrenError::DataTooLarge { .. })));
    }

    #[test]
    fn test_buffer_empty() {
        let capacity = 64;
        let (_buffer, ptr) = create_test_buffer(capacity);
        let header = unsafe { SharedHeader::init(ptr, capacity as u32) };
        let data_ptr = unsafe { ptr.add(SharedHeader::SIZE) };
        let ring = RingBuffer::new(header, data_ptr, capacity);

        assert!(matches!(ring.read(), Err(KrenError::BufferEmpty)));
    }

    #[test]
    fn test_wraparound() {
        let capacity = 64;
        let (_buffer, ptr) = create_test_buffer(capacity);
        let header = unsafe { SharedHeader::init(ptr, capacity as u32) };
        let data_ptr = unsafe { ptr.add(SharedHeader::SIZE) };
        let ring = RingBuffer::new(header, data_ptr, capacity);

        for i in 0..10 {
            let msg = format!("Iteration {}", i);
            ring.write(msg.as_bytes()).unwrap();
            let received = ring.read().unwrap();
            assert_eq!(received, msg.as_bytes());
        }
    }

    #[test]
    fn test_multiple_messages() {
        let capacity = 256;
        let (_buffer, ptr) = create_test_buffer(capacity);
        let header = unsafe { SharedHeader::init(ptr, capacity as u32) };
        let data_ptr = unsafe { ptr.add(SharedHeader::SIZE) };
        let ring = RingBuffer::new(header, data_ptr, capacity);

        let messages = ["First", "Second", "Third", "Fourth"];

        for msg in &messages {
            ring.write(msg.as_bytes()).unwrap();
        }

        for expected in &messages {
            let received = ring.read().unwrap();
            assert_eq!(received, expected.as_bytes());
        }
    }

    #[test]
    fn test_available_space() {
        let capacity = 128;
        let (_buffer, ptr) = create_test_buffer(capacity);
        let header = unsafe { SharedHeader::init(ptr, capacity as u32) };
        let data_ptr = unsafe { ptr.add(SharedHeader::SIZE) };
        let ring = RingBuffer::new(header, data_ptr, capacity);

        assert_eq!(ring.available_write(), capacity - 1);
        assert_eq!(ring.available_read(), 0);

        ring.write(b"Test").unwrap();
        assert!(ring.available_write() < capacity - 1);
        assert!(ring.available_read() > 0);

        ring.read().unwrap();
        assert_eq!(ring.available_write(), capacity - 1);
        assert_eq!(ring.available_read(), 0);
    }
}

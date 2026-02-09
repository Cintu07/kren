//! Lock-free SPSC ring buffer implementation
//!
//! This module provides a Single-Producer Single-Consumer ring buffer
//! optimized for inter-process communication over shared memory.

use crate::error::{KrenError, Result};
use crate::header::SharedHeader;
use std::ptr;

/// Message header for framed data in the ring buffer
/// 
/// Each message in the buffer is prefixed with a 4-byte length header.
const MSG_HEADER_SIZE: usize = 4;

/// Lock-free Single-Producer Single-Consumer ring buffer
/// 
/// This implementation uses atomic operations for head/tail synchronization,
/// allowing one writer and one reader to operate concurrently without locks.
/// 
/// # Memory Layout
/// 
/// ```text
/// [len:4][data:len][len:4][data:len]...
/// ^                ^
/// tail             head
/// ```
/// 
/// Each message is prefixed with its length (u32, little-endian).
pub struct RingBuffer {
    header: *mut SharedHeader,
    data: *mut u8,
    capacity: usize,
}

// Safety: RingBuffer is designed for cross-process use
// Synchronization is handled by atomic operations in SharedHeader
unsafe impl Send for RingBuffer {}
unsafe impl Sync for RingBuffer {}

impl RingBuffer {
    /// Create a new ring buffer view over shared memory
    /// 
    /// # Arguments
    /// * `header` - Pointer to the shared header (must be valid for lifetime of buffer)
    /// * `data` - Pointer to the data region
    /// * `capacity` - Size of the data region in bytes
    pub fn new(header: &mut SharedHeader, data: *mut u8, capacity: usize) -> Self {
        Self {
            header: header as *mut SharedHeader,
            data,
            capacity,
        }
    }

    /// Get a reference to the header
    #[inline]
    fn header(&self) -> &SharedHeader {
        unsafe { &*self.header }
    }

    /// Write data to the ring buffer
    /// 
    /// The data is prefixed with a 4-byte length header for framing.
    /// 
    /// # Arguments
    /// * `data` - Bytes to write
    /// 
    /// # Returns
    /// Number of bytes written (excluding length header), or error
    pub fn write(&self, data: &[u8]) -> Result<usize> {
        let len = data.len();
        let total_needed = MSG_HEADER_SIZE + len;

        // Check max message size
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

        // Write length header (little-endian)
        let len_bytes = (len as u32).to_le_bytes();
        self.write_at(head, &len_bytes);

        // Write data
        self.write_at((head + MSG_HEADER_SIZE) % self.capacity, data);

        // Update head atomically (makes write visible to reader)
        let new_head = (head + total_needed) % self.capacity;
        self.header().set_head(new_head as u32);

        Ok(len)
    }

    /// Read data from the ring buffer
    /// 
    /// # Returns
    /// The next message, or error if buffer is empty
    pub fn read(&self) -> Result<Vec<u8>> {
        if self.available_read() == 0 {
            return Err(KrenError::BufferEmpty);
        }

        let tail = self.header().tail() as usize;

        // Read length header
        let mut len_bytes = [0u8; 4];
        self.read_at(tail, &mut len_bytes);
        let len = u32::from_le_bytes(len_bytes) as usize;

        // Allocate and read data
        let mut data = vec![0u8; len];
        self.read_at((tail + MSG_HEADER_SIZE) % self.capacity, &mut data);

        // Update tail atomically (frees space for writer)
        let new_tail = (tail + MSG_HEADER_SIZE + len) % self.capacity;
        self.header().set_tail(new_tail as u32);

        Ok(data)
    }

    /// Calculate available space for writing
    pub fn available_write(&self) -> usize {
        let head = self.header().head() as usize;
        let tail = self.header().tail() as usize;

        if head >= tail {
            // [...tail...head...]
            // Available: capacity - (head - tail) - 1 (keep one byte gap)
            self.capacity - (head - tail) - 1
        } else {
            // [...head...tail...]
            // Available: tail - head - 1
            tail - head - 1
        }
    }

    /// Calculate available data for reading
    pub fn available_read(&self) -> usize {
        let head = self.header().head() as usize;
        let tail = self.header().tail() as usize;

        if head >= tail {
            head - tail
        } else {
            self.capacity - tail + head
        }
    }

    /// Write bytes at a position, handling wraparound
    fn write_at(&self, pos: usize, data: &[u8]) {
        let first_chunk = std::cmp::min(data.len(), self.capacity - pos);
        
        unsafe {
            // First chunk: from pos to end of buffer (or end of data)
            ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.data.add(pos),
                first_chunk,
            );

            // Second chunk: from start of buffer (if wrapped)
            if first_chunk < data.len() {
                ptr::copy_nonoverlapping(
                    data.as_ptr().add(first_chunk),
                    self.data,
                    data.len() - first_chunk,
                );
            }
        }
    }

    /// Read bytes from a position, handling wraparound
    fn read_at(&self, pos: usize, buf: &mut [u8]) {
        let first_chunk = std::cmp::min(buf.len(), self.capacity - pos);

        unsafe {
            // First chunk
            ptr::copy_nonoverlapping(
                self.data.add(pos),
                buf.as_mut_ptr(),
                first_chunk,
            );

            // Second chunk (if wrapped)
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
    use std::sync::atomic::{AtomicU32, AtomicU8};

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
        let written = ring.write(msg).expect("Write failed");
        assert_eq!(written, msg.len());

        let received = ring.read().expect("Read failed");
        assert_eq!(received, msg);
    }

    #[test]
    fn test_buffer_full() {
        let capacity = 64;
        let (_buffer, ptr) = create_test_buffer(capacity);
        
        let header = unsafe { SharedHeader::init(ptr, capacity as u32) };
        let data_ptr = unsafe { ptr.add(SharedHeader::SIZE) };
        let ring = RingBuffer::new(header, data_ptr, capacity);

        // Try to write more than capacity
        let big_data = vec![0u8; capacity];
        let result = ring.write(&big_data);
        assert!(matches!(result, Err(KrenError::DataTooLarge { .. })));
    }

    #[test]
    fn test_buffer_empty() {
        let capacity = 64;
        let (_buffer, ptr) = create_test_buffer(capacity);
        
        let header = unsafe { SharedHeader::init(ptr, capacity as u32) };
        let data_ptr = unsafe { ptr.add(SharedHeader::SIZE) };
        let ring = RingBuffer::new(header, data_ptr, capacity);

        let result = ring.read();
        assert!(matches!(result, Err(KrenError::BufferEmpty)));
    }

    #[test]
    fn test_wraparound() {
        let capacity = 64;
        let (_buffer, ptr) = create_test_buffer(capacity);
        
        let header = unsafe { SharedHeader::init(ptr, capacity as u32) };
        let data_ptr = unsafe { ptr.add(SharedHeader::SIZE) };
        let ring = RingBuffer::new(header, data_ptr, capacity);

        // Fill and drain several times to force wraparound
        for iteration in 0..10 {
            let msg = format!("Iteration {}", iteration);
            ring.write(msg.as_bytes()).expect("Write failed");
            let received = ring.read().expect("Read failed");
            assert_eq!(received, msg.as_bytes(), "Mismatch at iteration {}", iteration);
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
        
        // Write all
        for msg in &messages {
            ring.write(msg.as_bytes()).expect("Write failed");
        }

        // Read all
        for expected in &messages {
            let received = ring.read().expect("Read failed");
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

        // Initially almost full capacity available (minus 1 for gap)
        assert_eq!(ring.available_write(), capacity - 1);
        assert_eq!(ring.available_read(), 0);

        // Write some data
        ring.write(b"Test").expect("Write failed");
        assert!(ring.available_write() < capacity - 1);
        assert!(ring.available_read() > 0);

        // Read it back
        ring.read().expect("Read failed");
        assert_eq!(ring.available_write(), capacity - 1);
        assert_eq!(ring.available_read(), 0);
    }
}

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

pub const KREN_MAGIC: u32 = 0x4B52454E; // "KREN" in ASCII
pub const KREN_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChannelFlags {
    Active = 0,
    WriterClosed = 1,
    ReaderClosed = 2,
    Closed = 3,
}

impl From<u8> for ChannelFlags {
    fn from(value: u8) -> Self {
        match value {
            0 => ChannelFlags::Active,
            1 => ChannelFlags::WriterClosed,
            2 => ChannelFlags::ReaderClosed,
            _ => ChannelFlags::Closed,
        }
    }
}

/// Shared header structure placed at the beginning of the shared memory segment.
/// 
/// This struct uses `#[repr(C)]` to ensure consistent memory layout across
/// Rust, C, Python, and Node.js. All atomic fields use platform-native
/// atomic operations for lock-free synchronization.
/// 
/// Memory Layout (40 bytes total):
/// ```text
/// Offset  Size  Field
/// 0       4     magic
/// 4       4     version  
/// 8       4     capacity
/// 12      4     data_offset
/// 16      4     head (atomic)
/// 20      4     _pad1
/// 24      4     tail (atomic)
/// 28      4     _pad2
/// 32      1     flags (atomic)
/// 33      7     _pad3
/// ```
#[repr(C, align(8))]
pub struct SharedHeader {
    /// Magic number to identify valid KREN buffers (0x4B52454E)
    pub magic: u32,
    
    /// Protocol version for compatibility checking
    pub version: u32,
    
    /// Total capacity of the ring buffer in bytes
    pub capacity: u32,
    
    /// Offset from header start to data region
    pub data_offset: u32,
    
    /// Write index - controlled by producer (atomic for cross-process sync)
    pub head: AtomicU32,
    _pad1: u32,
    
    /// Read index - controlled by consumer (atomic for cross-process sync)
    pub tail: AtomicU32,
    _pad2: u32,
    
    /// Channel status flags (atomic)
    pub flags: AtomicU8,
    _pad3: [u8; 7],
}

impl SharedHeader {
    /// Size of the header in bytes
    pub const SIZE: usize = std::mem::size_of::<Self>();

    /// Initialize a new header at the given memory location
    /// 
    /// # Safety
    /// The pointer must point to valid, properly aligned memory
    /// of at least `SharedHeader::SIZE` bytes.
    pub unsafe fn init(ptr: *mut u8, capacity: u32) -> &'static mut Self {
        let header = &mut *(ptr as *mut Self);
        header.magic = KREN_MAGIC;
        header.version = KREN_VERSION;
        header.capacity = capacity;
        header.data_offset = Self::SIZE as u32;
        header.head = AtomicU32::new(0);
        header._pad1 = 0;
        header.tail = AtomicU32::new(0);
        header._pad2 = 0;
        header.flags = AtomicU8::new(ChannelFlags::Active as u8);
        header._pad3 = [0; 7];
        header
    }

    /// Get a reference to an existing header at the given memory location
    /// 
    /// # Safety
    /// The pointer must point to valid, properly aligned memory
    /// that was previously initialized with `SharedHeader::init()`.
    pub unsafe fn from_ptr(ptr: *mut u8) -> &'static mut Self {
        &mut *(ptr as *mut Self)
    }

    /// Validate that this is a valid KREN header
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.magic != KREN_MAGIC {
            return Err(crate::error::KrenError::InvalidMagic);
        }
        if self.version != KREN_VERSION {
            return Err(crate::error::KrenError::VersionMismatch {
                expected: KREN_VERSION,
                found: self.version,
            });
        }
        Ok(())
    }

    /// Get current head position (write index)
    #[inline]
    pub fn head(&self) -> u32 {
        self.head.load(Ordering::Acquire)
    }

    /// Get current tail position (read index)
    #[inline]
    pub fn tail(&self) -> u32 {
        self.tail.load(Ordering::Acquire)
    }

    /// Set head position (writer only)
    #[inline]
    pub fn set_head(&self, value: u32) {
        self.head.store(value, Ordering::Release);
    }

    /// Set tail position (reader only)
    #[inline]
    pub fn set_tail(&self, value: u32) {
        self.tail.store(value, Ordering::Release);
    }

    /// Get channel flags
    #[inline]
    pub fn get_flags(&self) -> ChannelFlags {
        ChannelFlags::from(self.flags.load(Ordering::Acquire))
    }

    /// Set channel flags
    #[inline]
    pub fn set_flags(&self, flags: ChannelFlags) {
        self.flags.store(flags as u8, Ordering::Release);
    }

    /// Check if channel is still active
    #[inline]
    pub fn is_active(&self) -> bool {
        self.get_flags() == ChannelFlags::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size_alignment() {
        // Header should be exactly 40 bytes with proper alignment
        assert_eq!(SharedHeader::SIZE, 40);
        assert_eq!(std::mem::align_of::<SharedHeader>(), 8);
    }

    #[test]
    fn test_header_init_and_validate() {
        let mut buffer = vec![0u8; SharedHeader::SIZE];
        let header = unsafe { SharedHeader::init(buffer.as_mut_ptr(), 4096) };
        
        assert_eq!(header.magic, KREN_MAGIC);
        assert_eq!(header.version, KREN_VERSION);
        assert_eq!(header.capacity, 4096);
        assert_eq!(header.data_offset, SharedHeader::SIZE as u32);
        assert_eq!(header.head(), 0);
        assert_eq!(header.tail(), 0);
        assert!(header.is_active());
        assert!(header.validate().is_ok());
    }

    #[test]
    fn test_invalid_magic() {
        let mut buffer = vec![0u8; SharedHeader::SIZE];
        let header = unsafe { SharedHeader::from_ptr(buffer.as_mut_ptr()) };
        
        let err = header.validate().unwrap_err();
        assert!(matches!(err, crate::error::KrenError::InvalidMagic));
    }

    #[test]
    fn test_atomic_operations() {
        let mut buffer = vec![0u8; SharedHeader::SIZE];
        let header = unsafe { SharedHeader::init(buffer.as_mut_ptr(), 1024) };
        
        header.set_head(100);
        header.set_tail(50);
        
        assert_eq!(header.head(), 100);
        assert_eq!(header.tail(), 50);
    }

    #[test]
    fn test_flags() {
        let mut buffer = vec![0u8; SharedHeader::SIZE];
        let header = unsafe { SharedHeader::init(buffer.as_mut_ptr(), 1024) };
        
        assert!(header.is_active());
        
        header.set_flags(ChannelFlags::WriterClosed);
        assert_eq!(header.get_flags(), ChannelFlags::WriterClosed);
        assert!(!header.is_active());
    }
}

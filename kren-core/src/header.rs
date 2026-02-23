use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

pub const KREN_MAGIC: u32 = 0x4B52454E; // "KREN"
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

// laid out for cross-language compatibility
// 40 bytes, 8-byte aligned
#[repr(C, align(8))]
pub struct SharedHeader {
    pub magic: u32,
    pub version: u32,
    pub capacity: u32,
    pub data_offset: u32,
    pub head: AtomicU32,
    _pad1: u32,
    pub tail: AtomicU32,
    _pad2: u32,
    pub flags: AtomicU8,
    _pad3: [u8; 7],
}

impl SharedHeader {
    pub const SIZE: usize = std::mem::size_of::<Self>();

    // safety: ptr must point to valid aligned memory of at least SIZE bytes
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

    // safety: ptr must point to a previously initialized header
    pub unsafe fn from_ptr(ptr: *mut u8) -> &'static mut Self {
        &mut *(ptr as *mut Self)
    }

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

    #[inline]
    pub fn head(&self) -> u32 {
        self.head.load(Ordering::Acquire)
    }

    #[inline]
    pub fn tail(&self) -> u32 {
        self.tail.load(Ordering::Acquire)
    }

    #[inline]
    pub fn set_head(&self, value: u32) {
        self.head.store(value, Ordering::Release);
    }

    #[inline]
    pub fn set_tail(&self, value: u32) {
        self.tail.store(value, Ordering::Release);
    }

    #[inline]
    pub fn get_flags(&self) -> ChannelFlags {
        ChannelFlags::from(self.flags.load(Ordering::Acquire))
    }

    #[inline]
    pub fn set_flags(&self, flags: ChannelFlags) {
        self.flags.store(flags as u8, Ordering::Release);
    }

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
        assert!(matches!(header.validate().unwrap_err(), crate::error::KrenError::InvalidMagic));
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

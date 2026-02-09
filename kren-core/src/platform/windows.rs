//! Windows shared memory implementation using Named File Mappings

use crate::error::{KrenError, Result};
use crate::platform::SharedMemory;
use std::ffi::c_void;
use std::ptr::NonNull;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile,
    FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
};

/// Windows shared memory implementation using Named File Mappings
/// 
/// This creates a named file mapping object that can be shared between processes.
/// The mapping is backed by the system page file, not a physical file.
pub struct WindowsSharedMemory {
    handle: HANDLE,
    ptr: NonNull<u8>,
    size: usize,
    name: String,
}

// Safety: The shared memory can be accessed from multiple threads
// The synchronization is handled by the atomic operations in SharedHeader
unsafe impl Send for WindowsSharedMemory {}
unsafe impl Sync for WindowsSharedMemory {}

impl WindowsSharedMemory {
    /// Convert a Rust string to a null-terminated wide string for Windows APIs
    fn to_wide_string(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Create the full mapping name with "Local\\" prefix
    fn make_mapping_name(name: &str) -> String {
        if name.starts_with("Local\\") || name.starts_with("Global\\") {
            name.to_string()
        } else {
            format!("Local\\kren_{}", name)
        }
    }
}

impl SharedMemory for WindowsSharedMemory {
    fn create(name: &str, size: usize) -> Result<Self> {
        let full_name = Self::make_mapping_name(name);
        let wide_name = Self::to_wide_string(&full_name);
        
        // Create file mapping backed by system page file (INVALID_HANDLE_VALUE)
        let handle = unsafe {
            CreateFileMappingW(
                HANDLE::default(), // Use page file
                None,              // Default security
                PAGE_READWRITE,    // Read/write access
                (size >> 32) as u32, // High 32 bits of size
                size as u32,       // Low 32 bits of size
                PCWSTR(wide_name.as_ptr()),
            )
        }.map_err(|e| KrenError::CreateFailed(e.to_string()))?;

        if handle.is_invalid() {
            return Err(KrenError::CreateFailed("CreateFileMappingW returned invalid handle".into()));
        }

        // Map the entire file mapping into our address space
        let map_result = unsafe {
            MapViewOfFile(
                handle,
                FILE_MAP_ALL_ACCESS,
                0, // Offset high
                0, // Offset low
                size,
            )
        };

        let ptr = match NonNull::new(map_result.Value as *mut u8) {
            Some(p) => p,
            None => {
                unsafe { let _ = CloseHandle(handle); }
                return Err(KrenError::MapFailed("MapViewOfFile returned null".into()));
            }
        };

        Ok(Self {
            handle,
            ptr,
            size,
            name: name.to_string(),
        })
    }

    fn open(name: &str) -> Result<Self> {
        let full_name = Self::make_mapping_name(name);
        let wide_name = Self::to_wide_string(&full_name);

        // Open existing file mapping
        let handle = unsafe {
            OpenFileMappingW(
                FILE_MAP_ALL_ACCESS.0,
                false, // Don't inherit handle
                PCWSTR(wide_name.as_ptr()),
            )
        }.map_err(|e| KrenError::OpenFailed(e.to_string()))?;

        if handle.is_invalid() {
            return Err(KrenError::OpenFailed("OpenFileMappingW returned invalid handle".into()));
        }

        // Map the file mapping - we don't know the size yet, so map everything
        let map_result = unsafe {
            MapViewOfFile(
                handle,
                FILE_MAP_ALL_ACCESS,
                0,
                0,
                0, // Map entire mapping
            )
        };

        let ptr = match NonNull::new(map_result.Value as *mut u8) {
            Some(p) => p,
            None => {
                unsafe { let _ = CloseHandle(handle); }
                return Err(KrenError::MapFailed("MapViewOfFile returned null".into()));
            }
        };

        // Read the size from the header
        let header = unsafe { &*(ptr.as_ptr() as *const crate::header::SharedHeader) };
        header.validate()?;
        let size = header.data_offset as usize + header.capacity as usize;

        Ok(Self {
            handle,
            ptr,
            size,
            name: name.to_string(),
        })
    }

    fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    fn size(&self) -> usize {
        self.size
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for WindowsSharedMemory {
    fn drop(&mut self) {
        unsafe {
            // Unmap the view first
            let addr = MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.ptr.as_ptr() as *mut c_void,
            };
            let _ = UnmapViewOfFile(addr);
            
            // Then close the handle
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::SharedHeader;

    #[test]
    fn test_create_and_open() {
        let name = "test_create_open";
        let size = SharedHeader::SIZE + 1024;

        // Create shared memory
        let shm1 = WindowsSharedMemory::create(name, size).expect("Failed to create");
        
        // Initialize header
        unsafe {
            SharedHeader::init(shm1.as_ptr(), 1024);
        }

        // Open from another "process" (same process, different handle)
        let shm2 = WindowsSharedMemory::open(name).expect("Failed to open");

        // Verify both see the same data
        let header1 = unsafe { SharedHeader::from_ptr(shm1.as_ptr()) };
        let header2 = unsafe { SharedHeader::from_ptr(shm2.as_ptr()) };

        assert_eq!(header1.magic, header2.magic);
        assert_eq!(header1.capacity, header2.capacity);

        // Write from shm1, read from shm2
        header1.set_head(42);
        assert_eq!(header2.head(), 42);
    }

    #[test]
    fn test_data_sharing() {
        let name = "test_data_sharing";
        let capacity = 256u32;
        let size = SharedHeader::SIZE + capacity as usize;

        let shm1 = WindowsSharedMemory::create(name, size).expect("Failed to create");
        unsafe { SharedHeader::init(shm1.as_ptr(), capacity); }

        let shm2 = WindowsSharedMemory::open(name).expect("Failed to open");

        // Write some data through shm1
        let data_ptr1 = unsafe { shm1.as_ptr().add(SharedHeader::SIZE) };
        let test_data = b"Hello, KREN!";
        unsafe {
            std::ptr::copy_nonoverlapping(test_data.as_ptr(), data_ptr1, test_data.len());
        }

        // Read through shm2
        let data_ptr2 = unsafe { shm2.as_ptr().add(SharedHeader::SIZE) };
        let mut read_buf = [0u8; 12];
        unsafe {
            std::ptr::copy_nonoverlapping(data_ptr2, read_buf.as_mut_ptr(), 12);
        }

        assert_eq!(&read_buf, test_data);
    }
}

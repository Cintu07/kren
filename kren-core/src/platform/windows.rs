use crate::error::{KrenError, Result};
use crate::platform::SharedMemory;
use std::ffi::c_void;
use std::ptr::{self, NonNull};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
    PAGE_READWRITE,
};

pub struct WindowsSharedMemory {
    handle: HANDLE,
    ptr: NonNull<u8>,
    size: usize,
    name: String,
}

unsafe impl Send for WindowsSharedMemory {}
unsafe impl Sync for WindowsSharedMemory {}

impl WindowsSharedMemory {
    fn encode_name(name: &str) -> Vec<u16> {
        let mut encoded: Vec<u16> = format!("Local\\KREN_{}", name).encode_utf16().collect();
        encoded.push(0);
        encoded
    }
}

impl SharedMemory for WindowsSharedMemory {
    fn create(name: &str, size: usize) -> Result<Self> {
        let name_encoded = Self::encode_name(name);
        
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                Some(ptr::null::<SECURITY_ATTRIBUTES>() as *const _),
                PAGE_READWRITE,
                (size >> 32) as u32,
                (size & 0xFFFFFFFF) as u32,
                PCWSTR::from_raw(name_encoded.as_ptr()),
            )
        }.map_err(|e| KrenError::CreateFailed(format!(
            "CreateFileMappingW failed: {}", e
        )))?;

        let ptr = unsafe {
            MapViewOfFile(
                handle,
                FILE_MAP_ALL_ACCESS,
                0,
                0,
                size,
            )
        };

        if ptr.Value.is_null() {
            unsafe { let _ = CloseHandle(handle); }
            return Err(KrenError::MapFailed(format!(
                "MapViewOfFile failed: {}", std::io::Error::last_os_error()
            )));
        }

        let ptr = NonNull::new(ptr.Value as *mut u8).unwrap();

        Ok(Self {
            handle,
            ptr,
            size,
            name: name.to_string(),
        })
    }

    fn open(name: &str) -> Result<Self> {
        let name_encoded = Self::encode_name(name);

        let handle = unsafe {
            OpenFileMappingW(
                FILE_MAP_ALL_ACCESS.0,
                false,
                PCWSTR::from_raw(name_encoded.as_ptr()),
            )
        }.map_err(|e| KrenError::OpenFailed(format!(
            "OpenFileMappingW failed: {}", e
        )))?;

        let ptr = unsafe {
            MapViewOfFile(
                handle,
                FILE_MAP_ALL_ACCESS,
                0,
                0,
                0,
            )
        };

        if ptr.Value.is_null() {
            unsafe { let _ = CloseHandle(handle); }
            return Err(KrenError::MapFailed(format!(
                "MapViewOfFile failed: {}", std::io::Error::last_os_error()
            )));
        }

        let ptr = NonNull::new(ptr.Value as *mut u8).unwrap();

        let header = unsafe { &*(ptr.as_ptr() as *const crate::header::SharedHeader) };
        header.validate()?;
        let total_size = header.data_offset as usize + header.capacity as usize;

        Ok(Self {
            handle,
            ptr,
            size: total_size,
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
            let _ = UnmapViewOfFile(windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS { Value: self.ptr.as_ptr() as *mut c_void });
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
        let name = "test_create_win";
        let size = SharedHeader::SIZE + 1024;

        let shm1 = WindowsSharedMemory::create(name, size).unwrap();
        unsafe { SharedHeader::init(shm1.as_ptr(), 1024); }

        let shm2 = WindowsSharedMemory::open(name).unwrap();

        let header1 = unsafe { SharedHeader::from_ptr(shm1.as_ptr()) };
        let header2 = unsafe { SharedHeader::from_ptr(shm2.as_ptr()) };

        assert_eq!(header1.magic, header2.magic);
        header1.set_head(42);
        assert_eq!(header2.head(), 42);
    }
}

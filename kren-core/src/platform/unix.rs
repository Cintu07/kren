//! Unix/macOS shared memory implementation using POSIX shm_open/mmap

use crate::error::{KrenError, Result};
use crate::platform::SharedMemory;
use std::ffi::CString;
use std::ptr::{self, NonNull};

/// Unix shared memory implementation using POSIX shared memory
///
/// Uses shm_open to create named shared memory objects and mmap to map
/// them into the process address space. Works on Linux and macOS.
pub struct UnixSharedMemory {
    fd: libc::c_int,
    ptr: NonNull<u8>,
    size: usize,
    name: String,
    shm_name: CString,
    is_owner: bool,
}

// Safety: The shared memory can be accessed from multiple threads
// Synchronization is handled by atomic operations in SharedHeader
unsafe impl Send for UnixSharedMemory {}
unsafe impl Sync for UnixSharedMemory {}

impl UnixSharedMemory {
    /// Create the POSIX shared memory name with /kren_ prefix
    fn make_shm_name(name: &str) -> CString {
        let full = if name.starts_with('/') {
            name.to_string()
        } else {
            format!("/kren_{}", name)
        };
        CString::new(full).expect("Invalid shared memory name")
    }
}

impl SharedMemory for UnixSharedMemory {
    fn create(name: &str, size: usize) -> Result<Self> {
        let shm_name = Self::make_shm_name(name);

        // Create shared memory object
        let fd = unsafe {
            libc::shm_open(
                shm_name.as_ptr(),
                libc::O_CREAT | libc::O_RDWR | libc::O_EXCL,
                0o600,
            )
        };

        // If it already exists, unlink and retry
        let fd = if fd == -1 {
            unsafe { libc::shm_unlink(shm_name.as_ptr()); }
            let fd = unsafe {
                libc::shm_open(
                    shm_name.as_ptr(),
                    libc::O_CREAT | libc::O_RDWR,
                    0o600,
                )
            };
            if fd == -1 {
                return Err(KrenError::CreateFailed(
                    format!("shm_open failed: {}", std::io::Error::last_os_error())
                ));
            }
            fd
        } else {
            fd
        };

        // Set the size
        if unsafe { libc::ftruncate(fd, size as libc::off_t) } == -1 {
            unsafe {
                libc::close(fd);
                libc::shm_unlink(shm_name.as_ptr());
            }
            return Err(KrenError::CreateFailed(
                format!("ftruncate failed: {}", std::io::Error::last_os_error())
            ));
        }

        // Map into address space
        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            unsafe {
                libc::close(fd);
                libc::shm_unlink(shm_name.as_ptr());
            }
            return Err(KrenError::MapFailed(
                format!("mmap failed: {}", std::io::Error::last_os_error())
            ));
        }

        let ptr = match NonNull::new(ptr as *mut u8) {
            Some(p) => p,
            None => {
                unsafe {
                    libc::close(fd);
                    libc::shm_unlink(shm_name.as_ptr());
                }
                return Err(KrenError::MapFailed("mmap returned null".into()));
            }
        };

        Ok(Self {
            fd,
            ptr,
            size,
            name: name.to_string(),
            shm_name,
            is_owner: true,
        })
    }

    fn open(name: &str) -> Result<Self> {
        let shm_name = Self::make_shm_name(name);

        // Open existing shared memory
        let fd = unsafe {
            libc::shm_open(
                shm_name.as_ptr(),
                libc::O_RDWR,
                0,
            )
        };

        if fd == -1 {
            return Err(KrenError::OpenFailed(
                format!("shm_open failed: {}", std::io::Error::last_os_error())
            ));
        }

        // Get the size from the file descriptor
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut stat) } == -1 {
            unsafe { libc::close(fd); }
            return Err(KrenError::OpenFailed(
                format!("fstat failed: {}", std::io::Error::last_os_error())
            ));
        }

        let size = stat.st_size as usize;

        // Map into address space
        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            unsafe { libc::close(fd); }
            return Err(KrenError::MapFailed(
                format!("mmap failed: {}", std::io::Error::last_os_error())
            ));
        }

        let ptr = match NonNull::new(ptr as *mut u8) {
            Some(p) => p,
            None => {
                unsafe { libc::close(fd); }
                return Err(KrenError::MapFailed("mmap returned null".into()));
            }
        };

        // Validate header
        let header = unsafe { &*(ptr.as_ptr() as *const crate::header::SharedHeader) };
        header.validate()?;
        let total_size = header.data_offset as usize + header.capacity as usize;

        Ok(Self {
            fd,
            ptr,
            size: total_size,
            name: name.to_string(),
            shm_name,
            is_owner: false,
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

impl Drop for UnixSharedMemory {
    fn drop(&mut self) {
        unsafe {
            // Unmap the memory
            libc::munmap(self.ptr.as_ptr() as *mut libc::c_void, self.size);

            // Close the file descriptor
            libc::close(self.fd);

            // Only the owner unlinks (removes) the shared memory
            if self.is_owner {
                libc::shm_unlink(self.shm_name.as_ptr());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::SharedHeader;

    #[test]
    fn test_create_and_open() {
        let name = "test_unix_create";
        let size = SharedHeader::SIZE + 1024;

        let shm1 = UnixSharedMemory::create(name, size).expect("Failed to create");

        unsafe {
            SharedHeader::init(shm1.as_ptr(), 1024);
        }

        let shm2 = UnixSharedMemory::open(name).expect("Failed to open");

        let header1 = unsafe { SharedHeader::from_ptr(shm1.as_ptr()) };
        let header2 = unsafe { SharedHeader::from_ptr(shm2.as_ptr()) };

        assert_eq!(header1.magic, header2.magic);
        assert_eq!(header1.capacity, header2.capacity);

        header1.set_head(42);
        assert_eq!(header2.head(), 42);
    }

    #[test]
    fn test_data_sharing() {
        let name = "test_unix_data";
        let capacity = 256u32;
        let size = SharedHeader::SIZE + capacity as usize;

        let shm1 = UnixSharedMemory::create(name, size).expect("Failed to create");
        unsafe { SharedHeader::init(shm1.as_ptr(), capacity); }

        let shm2 = UnixSharedMemory::open(name).expect("Failed to open");

        let data_ptr1 = unsafe { shm1.as_ptr().add(SharedHeader::SIZE) };
        let test_data = b"Hello, KREN!";
        unsafe {
            std::ptr::copy_nonoverlapping(test_data.as_ptr(), data_ptr1, test_data.len());
        }

        let data_ptr2 = unsafe { shm2.as_ptr().add(SharedHeader::SIZE) };
        let mut read_buf = [0u8; 12];
        unsafe {
            std::ptr::copy_nonoverlapping(data_ptr2, read_buf.as_mut_ptr(), 12);
        }

        assert_eq!(&read_buf, test_data);
    }
}

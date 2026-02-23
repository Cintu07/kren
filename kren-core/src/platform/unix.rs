use crate::error::{KrenError, Result};
use crate::platform::SharedMemory;
use std::ffi::CString;
use std::ptr::{self, NonNull};

pub struct UnixSharedMemory {
    fd: libc::c_int,
    ptr: NonNull<u8>,
    size: usize,
    name: String,
    shm_name: CString,
    is_owner: bool,
}

unsafe impl Send for UnixSharedMemory {}
unsafe impl Sync for UnixSharedMemory {}

impl UnixSharedMemory {
    fn make_shm_name(name: &str) -> CString {
        let full = if name.starts_with('/') {
            name.to_string()
        } else {
            format!("/kren_{}", name)
        };
        CString::new(full).unwrap()
    }
}

impl SharedMemory for UnixSharedMemory {
    fn create(name: &str, size: usize) -> Result<Self> {
        let shm_name = Self::make_shm_name(name);

        let fd = unsafe {
            libc::shm_open(
                shm_name.as_ptr(),
                libc::O_CREAT | libc::O_RDWR | libc::O_EXCL,
                0o600,
            )
        };

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

        if unsafe { libc::ftruncate(fd, size as libc::off_t) } == -1 {
            unsafe {
                libc::close(fd);
                libc::shm_unlink(shm_name.as_ptr());
            }
            return Err(KrenError::CreateFailed(
                format!("ftruncate failed: {}", std::io::Error::last_os_error())
            ));
        }

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

        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut stat) } == -1 {
            unsafe { libc::close(fd); }
            return Err(KrenError::OpenFailed(
                format!("fstat failed: {}", std::io::Error::last_os_error())
            ));
        }

        let size = stat.st_size as usize;

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
            libc::munmap(self.ptr.as_ptr() as *mut libc::c_void, self.size);
            libc::close(self.fd);
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

        let shm1 = UnixSharedMemory::create(name, size).unwrap();
        unsafe { SharedHeader::init(shm1.as_ptr(), 1024); }

        let shm2 = UnixSharedMemory::open(name).unwrap();

        let header1 = unsafe { SharedHeader::from_ptr(shm1.as_ptr()) };
        let header2 = unsafe { SharedHeader::from_ptr(shm2.as_ptr()) };

        assert_eq!(header1.magic, header2.magic);
        header1.set_head(42);
        assert_eq!(header2.head(), 42);
    }
}

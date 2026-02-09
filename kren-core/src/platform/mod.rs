//! Platform-specific shared memory implementations

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::*;

use crate::error::Result;

/// Trait for platform-specific shared memory implementations
pub trait SharedMemory: Sized {
    /// Create a new shared memory segment with the given name and size
    fn create(name: &str, size: usize) -> Result<Self>;
    
    /// Open an existing shared memory segment by name
    fn open(name: &str) -> Result<Self>;
    
    /// Get a raw pointer to the mapped memory
    fn as_ptr(&self) -> *mut u8;
    
    /// Get the size of the mapped memory
    fn size(&self) -> usize;
    
    /// Get the name of the shared memory segment
    fn name(&self) -> &str;
}

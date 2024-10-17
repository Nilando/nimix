use super::constants::BLOCK_SIZE;
use super::error::AllocError;
use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

pub struct Block {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl Block {
    pub fn default() -> Result<Block, AllocError> {
        let layout = Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).unwrap();

        Self::new(layout)
    }

    pub fn new(layout: Layout) -> Result<Block, AllocError> {
        Ok(Block {
            ptr: Self::alloc_block(layout)?,
            layout,
        })
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    pub fn get_size(&self) -> usize {
        self.layout.size()
    }

    fn alloc_block(layout: Layout) -> Result<NonNull<u8>, AllocError> {
        let ptr = unsafe { alloc(layout) };

        match NonNull::new(ptr) {
            Some(ptr) => Ok(ptr),
            None => Err(AllocError::OOM),
        }
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        unsafe { dealloc(self.ptr.as_ptr(), self.layout) }
    }
}

mod alloc_head;
mod block;
mod block_store;
mod bump_block;
mod error;
mod large_block;
mod size_class;
mod constants;

use alloc_head::AllocHead;
use block_store::BlockStore;
use large_block::LargeBlock;
use size_class::SizeClass;
use std::num::NonZero;
use std::alloc::Layout;
use std::sync::Arc;

pub use error::AllocError;

use crate::block::Block;
use crate::constants::{BLOCK_SIZE, META_CAPACITY};

#[derive(Clone)]
pub struct Heap {
    head: AllocHead
}

impl Heap {
    pub fn new() -> Self {
        let store = Arc::new(BlockStore::new());

        Self {
            head: AllocHead::new(store),
        }
    }
    pub unsafe fn alloc(&self, layout: Layout) -> Result<*mut u8, AllocError> {
        let ptr = self.head.alloc(layout)?;

        Ok(ptr as *mut u8)
    }

    pub unsafe fn sweep(&self, mark: NonZero<u8>, cb: impl FnOnce()) {
        self.head.sweep(mark.into(), cb);
    }

    pub fn size(&self) -> usize {
        self.head.get_size()
    }

    pub unsafe fn mark(ptr: *mut u8, layout: Layout, mark: NonZero<u8>) -> Result<(), AllocError> {
        let size_class =  SizeClass::get_for_size(layout.size())?;

        if size_class != SizeClass::Large {
            let block = Block::from_ptr(ptr);
            let idx = (ptr as usize % BLOCK_SIZE) - META_CAPACITY;

            block.mark_object(idx, layout.size() as u32, size_class, mark.into());

            Ok(())
        } else {
            LargeBlock::mark(ptr, layout, mark.into())
        }
    }
}

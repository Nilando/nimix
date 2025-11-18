#![no_std]
extern crate alloc;

mod atomic_stack;
mod allocator;
mod block;
mod block_store;
mod bump_block;
mod error;
mod large_block;
mod size_class;
mod constants;
mod heap;

use large_block::LargeBlock;
use size_class::SizeClass;
use core::num::NonZero;
use alloc::alloc::Layout;

use crate::block::Block;

// PUBLIC API BELOW

pub use heap::Heap;
pub use allocator::Allocator;
pub use error::AllocError;

pub unsafe fn mark(ptr: *const u8, layout: Layout, mark: NonZero<u8>) -> Result<(), AllocError> {
    let size_class =  SizeClass::get_for_size(layout.size())?;

    if size_class != SizeClass::Large {
        Block::mark(ptr, layout, size_class, mark);

        Ok(())
    } else {
        LargeBlock::mark(ptr, layout, mark)
    }
}

mod alloc_head;
mod block;
mod block_meta;
mod block_store;
mod bump_block;
mod error;
mod large_block;
mod size_class;

pub mod constants;

use alloc_head::AllocHead;
use block_meta::BlockMeta;
use large_block::LargeBlock;
use size_class::SizeClass;
use std::num::NonZero;
use std::alloc::Layout;
use block_store::BLOCK_STORE;

pub use error::AllocError;

std::thread_local!(static ALLOC_HEAD: AllocHead = AllocHead::new());

pub unsafe fn alloc(layout: Layout) -> Result<*mut u8, AllocError> {
    let ptr = ALLOC_HEAD.with(|head| head.alloc(layout))?;

    Ok(ptr as *mut u8)
}

pub unsafe fn mark(ptr: *mut u8, layout: Layout, mark: NonZero<u8>) -> Result<(), AllocError> {
    let size_class =  SizeClass::get_for_size(layout.size())?;

    if size_class != SizeClass::Large {
        let meta = BlockMeta::from_ptr(ptr);

        meta.mark(ptr, layout.size() as u32, size_class, mark.into());
    } else {
        LargeBlock::mark(ptr, layout, mark.into())?;
    }

    Ok(())
}

pub unsafe fn sweep<F: FnOnce()>(mark: NonZero<u8>, cb: F) {
    BLOCK_STORE.sweep(mark.into(), cb);
}

pub fn get_size() -> usize {
    BLOCK_STORE.get_size()
}

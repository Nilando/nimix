mod alloc_head;
mod block;
mod block_meta;
mod block_store;
mod bump_block;
mod constants;
mod error;
mod size_class;

use alloc_head::AllocHead;
use block_meta::BlockMeta;
use block_store::BlockStore;
pub use error::AllocError;
use size_class::SizeClass;
use std::num::NonZero;
use std::alloc::Layout;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::sync::LazyLock;

static BLOCK_STORE: LazyLock<Arc<BlockStore>> = LazyLock::new(|| Arc::new(BlockStore::new()));

std::thread_local!(static ALLOC_HEAD: AllocHead = AllocHead::new(BLOCK_STORE.clone()));

pub unsafe fn alloc(layout: Layout) -> Result<*mut u8, AllocError> {
    let ptr = ALLOC_HEAD.with(|head| head.alloc(layout))?;

    Ok(ptr as *mut u8)
}

pub unsafe fn mark(ptr: *mut u8, layout: Layout, mark: NonZero<u8>) -> Result<(), AllocError> {
    if SizeClass::get_for_size(layout.size())? != SizeClass::Large {
        let meta = BlockMeta::from_ptr(ptr);

        meta.mark(ptr, layout.size() as u32, mark.into());
    } else {
        let header_layout = Layout::new::<AllocMark>();
        let (_alloc_layout, obj_offset) = header_layout
            .extend(layout)
            .expect("todo: turn this into an alloc error");
        let block_ptr: &AtomicU8 = unsafe { &*ptr.sub(obj_offset).cast() };

        block_ptr.store(mark.into(), Ordering::Release)
    }

    Ok(())
}

pub unsafe fn sweep<F: FnOnce()>(mark: NonZero<u8>, cb: F) {
    BLOCK_STORE.sweep(mark.into(), cb);
}

pub fn get_size() -> usize {
    BLOCK_STORE.get_size()
}

type AllocMark = AtomicU8;

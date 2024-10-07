use super::alloc_head::AllocHead;
use super::block_meta::BlockMeta;
use super::block_store::BlockStore;
use super::error::AllocError;
use super::size_class::SizeClass;
use std::num::NonZero;
use std::alloc::Layout;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

pub type AllocMark = AtomicU8;

#[derive(Clone)]
pub struct Allocator {
    head: AllocHead,
}

impl Allocator {
    pub fn new() -> Self {
        let block_store = BlockStore::new();

        Self {
            head: AllocHead::new(Arc::new(block_store)),
        }
    }

    pub unsafe fn alloc(&self, layout: Layout) -> Result<*mut u8, AllocError> {
        let ptr = self.head.alloc(layout)?;

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

    // callback is called once the sweep is initialized
    pub unsafe fn sweep<F: FnOnce()>(&self, mark: NonZero<u8>, cb: F) 
    {
        self.head.sweep(mark.into(), cb);
    }

    pub fn get_size(&self) -> usize {
        self.head.get_size()
    }
}

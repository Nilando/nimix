use core::num::NonZero;
use alloc::sync::Arc;

use crate::{block_store::BlockStore, Allocator};

impl From<&Heap> for Allocator {
    fn from(heap: &Heap) -> Self {
        Allocator::new(heap.store.clone())
    }
}

pub struct Heap {
    store: Arc<BlockStore>,
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl Heap {
    pub fn new() -> Self {
        let store = Arc::new(BlockStore::new());

        Self {
            store
        }
    }

    pub fn size(&self) -> usize {
        self.store.get_size()
    }

    pub unsafe fn sweep(&self, live_mark: NonZero<u8>) {
        self.store.sweep(live_mark);
    }
}

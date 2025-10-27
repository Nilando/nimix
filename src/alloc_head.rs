use super::block_store::BlockStore;
use super::bump_block::BumpBlock;
use super::error::AllocError;
use super::size_class::SizeClass;
use alloc::alloc::Layout;
use core::cell::Cell;
use alloc::sync::Arc;
use core::num::NonZero;

pub struct AllocHead {
    head: Cell<Option<BumpBlock>>,
    overflow: Cell<Option<BumpBlock>>,
    store: Arc<BlockStore>,
}

impl Drop for AllocHead {
    fn drop(&mut self) {
        self.flush()
    }
}

impl Clone for AllocHead {
    fn clone(&self) -> Self {
        Self {
            head: Cell::new(None),
            overflow: Cell::new(None),
            store: self.store.clone()
        }
    }
}

impl AllocHead {
    pub const fn new(store: Arc<BlockStore>) -> Self {
        Self {
            head: Cell::new(None),
            overflow: Cell::new(None),
            store,
        }
    }

    pub fn alloc(&self, layout: Layout) -> Result<*const u8, AllocError> {
        let size_class = SizeClass::get_for_size(layout.size())?;

        match size_class {
            SizeClass::Small => self.small_alloc(layout),
            SizeClass::Medium => self.medium_alloc(layout),
            SizeClass::Large => self.store.create_large(layout),
        }
    }

    pub fn sweep(&self, mark: NonZero<u8>, cb: impl FnOnce()) {
        self.store.sweep(mark.into(), cb);
    }

    pub fn get_size(&self) -> usize {
        self.store.get_size()
    }

    fn small_alloc(&self, layout: Layout) -> Result<*const u8, AllocError> {
        loop {
            if let Some(ptr) = self.head_alloc(layout) {
                return Ok(ptr);
            }

            self.get_new_head()?;
        }
    }

    fn medium_alloc(&self, layout: Layout) -> Result<*const u8, AllocError> {
        loop {
            if let Some(space) = self.overflow_alloc(layout) {
                return Ok(space);
            }

            self.get_new_overflow()?;
        }
    }

    fn get_new_head(&self) -> Result<(), AllocError> {
        let new_head = match self.overflow.take() {
            Some(block) => block,
            None => self.store.get_head()?,
        };

        let rest_block = self.head.take();
        self.head.set(Some(new_head));

        if let Some(block) = rest_block {
            self.store.rest(block);
        }

        Ok(())
    }

    fn get_new_overflow(&self) -> Result<(), AllocError> {
        let new_overflow = self.store.get_overflow()?;
        let recycle_block = self.overflow.take();

        self.overflow.set(Some(new_overflow));

        if let Some(block) = recycle_block {
            self.store.recycle(block);
        }

        Ok(())
    }

    fn head_alloc(&self, layout: Layout) -> Option<*const u8> {
        match self.head.take() {
            Some(mut head) => {
                let result = head.inner_alloc(layout);
                self.head.set(Some(head));
                result
            }
            None => None,
        }
    }

    fn overflow_alloc(&self, layout: Layout) -> Option<*const u8> {
        match self.overflow.take() {
            Some(mut overflow) => {
                let result = overflow.inner_alloc(layout);
                self.overflow.set(Some(overflow));
                result
            }
            None => None,
        }
    }

    fn flush(&self)  {
        if let Some(head) = self.head.take() {
            self.store.recycle(head);
        }

        if let Some(overflow) = self.overflow.take() {
            self.store.recycle(overflow);
        }
    }
}

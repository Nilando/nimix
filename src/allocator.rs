use super::block_store::BlockStore;
use super::bump_block::BumpBlock;
use super::error::AllocError;
use super::size_class::SizeClass;
use alloc::alloc::Layout;
use core::cell::Cell;
use alloc::sync::Arc;

pub struct Allocator {
    head: Cell<Option<BumpBlock>>,
    overflow: Cell<Option<BumpBlock>>,
    store: Arc<BlockStore>,
}

impl Drop for Allocator {
    fn drop(&mut self) {
        self.flush()
    }
}

impl Allocator {
    pub(crate) fn new(store: Arc<BlockStore>) -> Self {
        Self {
            head: Cell::new(None),
            overflow: Cell::new(None),
            store,
        }
    }

    pub unsafe fn alloc(&self, layout: Layout) -> Result<*const u8, AllocError> {
        // Debug: validate layout parameters
        assert!(layout.size() > 0, "alloc: size must be > 0");

        let size_class = SizeClass::get_for_size(layout.size())?;

        let ptr = match size_class {
            SizeClass::Small => self.small_alloc(layout),
            SizeClass::Medium => self.medium_alloc(layout),
            SizeClass::Large => self.store.create_large(layout),
        }?;

        // Debug: validate returned pointer
        debug_assert!(!ptr.is_null(), "alloc: returned null pointer");
        debug_assert_eq!(
            ptr as usize % layout.align(),
            0,
            "alloc: returned pointer {:p} is not aligned to {} (offset: {})",
            ptr,
            layout.align(),
            ptr as usize % layout.align()
        );

        Ok(ptr)
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

    fn head_alloc(&self, layout: Layout) -> Option<*mut u8> {
        match self.head.take() {
            Some(mut head) => {
                let result = head.inner_alloc(layout);
                self.head.set(Some(head));
                result
            }
            None => None,
        }
    }

    fn overflow_alloc(&self, layout: Layout) -> Option<*mut u8> {
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

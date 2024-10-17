use super::block_store::BLOCK_STORE;
use super::bump_block::BumpBlock;
use super::error::AllocError;
use super::size_class::SizeClass;
use std::alloc::Layout;
use std::cell::Cell;

pub struct AllocHead {
    head: Cell<Option<BumpBlock>>,
    overflow: Cell<Option<BumpBlock>>,
}

impl Drop for AllocHead {
    fn drop(&mut self) {
        self.flush()
    }
}

impl AllocHead {
    pub const fn new() -> Self {
        Self {
            head: Cell::new(None),
            overflow: Cell::new(None),
        }
    }

    // maybe this could be public?
    pub fn flush(&self)  {
        if let Some(head) = self.head.take() {
            BLOCK_STORE.recycle(head);
        }

        if let Some(overflow) = self.overflow.take() {
            BLOCK_STORE.recycle(overflow);
        }
    }

    pub fn alloc(&self, layout: Layout) -> Result<*const u8, AllocError> {
        let size_class = SizeClass::get_for_size(layout.size())?;

        match size_class {
            SizeClass::Small => self.small_alloc(layout),
            SizeClass::Medium => self.medium_alloc(layout),
            SizeClass::Large => BLOCK_STORE.create_large(layout),
        }
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
            None => BLOCK_STORE.get_head()?,
        };

        let rest_block = self.head.take();
        self.head.set(Some(new_head));

        if let Some(block) = rest_block {
            BLOCK_STORE.rest(block);
        }

        Ok(())
    }

    fn get_new_overflow(&self) -> Result<(), AllocError> {
        let new_overflow = BLOCK_STORE.get_overflow()?;
        let recycle_block = self.overflow.take();

        self.overflow.set(Some(new_overflow));

        if let Some(block) = recycle_block {
            BLOCK_STORE.recycle(block);
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
}

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

impl Clone for AllocHead {
    fn clone(&self) -> Self {
        Self {
            head: Cell::new(None),
            overflow: Cell::new(None),
        }
    }
}

impl Drop for AllocHead {
    fn drop(&mut self) {
        self.flush()
    }
}

impl AllocHead {
    pub fn new() -> Self {
        Self {
            head: Cell::new(None),
            overflow: Cell::new(None),
        }
    }

    // maybe this could be public?
    fn flush(&self)  {
        if let Some(head) = self.head.take() {
            BLOCK_STORE.push_recycle(head);
        }

        if let Some(overflow) = self.overflow.take() {
            BLOCK_STORE.push_recycle(overflow);
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
        // this is okay be we already tried to alloc in head and didn't have space
        // and any block returned by get new head should have space for a small object
        loop {
            self.get_new_head()?;

            if let Some(ptr) = self.head_alloc(layout) {
                return Ok(ptr);
            }
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
            BLOCK_STORE.push_rest(block);
        }

        Ok(())
    }

    fn get_new_overflow(&self) -> Result<(), AllocError> {
        let new_overflow = BLOCK_STORE.get_overflow()?;
        let recycle_block = self.overflow.take();
        self.overflow.set(Some(new_overflow));

        if let Some(block) = recycle_block {
            BLOCK_STORE.push_recycle(block);
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

#[cfg(test)]
mod tests {
    use super::super::constants;
    use super::*;

    #[test]
    fn test_recycle_alloc() {
        let blocks = AllocHead::new();
        let medium_layout =
            Layout::from_size_align(constants::BLOCK_CAPACITY - constants::LINE_SIZE, 8).unwrap();
        let small_layout = Layout::from_size_align(constants::LINE_SIZE, 8).unwrap();

        blocks.alloc(medium_layout).unwrap();
        //assert_eq!(store.block_count(), 1);

        blocks.alloc(medium_layout).unwrap();
        //assert_eq!(store.block_count(), 2);

        blocks.alloc(medium_layout).unwrap();
        //assert_eq!(store.block_count(), 3);

        // this alloc should alloc should fill the head
        blocks.alloc(small_layout).unwrap();
        //assert_eq!(store.block_count(), 3);

        // this alloc should alloc should fill the overflow head
        blocks.alloc(small_layout).unwrap();
        //assert_eq!(store.block_count(), 3);

        // this alloc should alloc should fill the recycle
        blocks.alloc(small_layout).unwrap();
        //assert_eq!(store.block_count(), 3);

        // this alloc should alloc should need a new block
        blocks.alloc(small_layout).unwrap();
        //assert_eq!(store.block_count(), 4);
    }

    #[test]
    fn test_alloc_many_blocks() {
        let blocks = AllocHead::new();
        let medium_layout = Layout::from_size_align(constants::BLOCK_CAPACITY, 8).unwrap();

        for _ in 1..100 {
            blocks.alloc(medium_layout).unwrap();
            //assert_eq!(store.block_count(), i);
        }
    }

    #[test]
    fn test_alloc_into_overflow() {
        let blocks = AllocHead::new();
        let medium_layout = Layout::from_size_align(constants::BLOCK_CAPACITY, 8).unwrap();
        let medium_layout_2 = Layout::from_size_align(constants::BLOCK_CAPACITY / 2, 8).unwrap();

        blocks.alloc(medium_layout).unwrap();
        blocks.alloc(medium_layout_2).unwrap();
        blocks.alloc(medium_layout_2).unwrap();
        //assert_eq!(store.block_count(), 2);

        blocks.alloc(medium_layout_2).unwrap();
        blocks.alloc(medium_layout_2).unwrap();
        //assert_eq!(store.block_count(), 3);
    }

    #[test]
    fn medium_and_small_allocs() {
        let blocks = AllocHead::new();
        let medium_layout = Layout::new::<[u8; constants::LINE_SIZE * 2]>();
        let small_layout = Layout::from_size_align(constants::LINE_SIZE, 8).unwrap();
        let mut small_ptrs = Vec::<*const u8>::new();
        let mut med_ptrs = Vec::<*const u8>::new();

        for _ in 0..2000 {
            let ptr = blocks.alloc(small_layout).unwrap();
            small_ptrs.push(ptr);

            let med_ptr = blocks.alloc(medium_layout).unwrap();
            med_ptrs.push(med_ptr);
        }

        while let Some(ptr) = small_ptrs.pop() {
            assert!(!med_ptrs.contains(&ptr));
            assert!(!small_ptrs.contains(&ptr));
        }
    }
}

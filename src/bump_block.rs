use super::block::Block;
use super::block_meta::BlockMeta;
use super::constants::{BLOCK_CAPACITY, SMALL_OBJECT_MIN};
use super::error::AllocError;
use std::alloc::Layout;
use std::num::NonZero;

pub struct BumpBlock {
    cursor: usize,
    limit: usize,
    block: Block,
    meta: BlockMeta,
}

impl BumpBlock {
    pub fn new() -> Result<BumpBlock, AllocError> {
        let block = Block::default()?;
        let meta = BlockMeta::new(&block);
        let bump_block = BumpBlock {
            cursor: BLOCK_CAPACITY,
            limit: 0,
            block,
            meta
        };

        Ok(bump_block)
    }

    pub fn reset_hole(&mut self, mark: NonZero<u8>) {
        self.meta.free_unmarked(mark);

        if self.meta.get_block_mark() != mark.into() {
            self.cursor = BLOCK_CAPACITY;
            self.limit = 0;
            return;
        }

        if let Some((cursor, limit)) = self
            .meta
            .find_next_available_hole(BLOCK_CAPACITY, SMALL_OBJECT_MIN)
        {
            self.cursor = cursor;
            self.limit = limit;
        } else {
            self.cursor = 0;
            self.limit = 0;
        }
    }

    pub fn inner_alloc(&mut self, layout: Layout) -> Option<*const u8> {
        loop {
            let next = self.cursor.checked_sub(layout.size())? & !(layout.align() - 1);

            if self.limit <= next {
                self.cursor = next;

                let ptr = unsafe { self.block.as_ptr().add(self.cursor) };

                debug_assert!(self.block.as_ptr() as usize <= ptr as usize);
                debug_assert!(self.block.as_ptr() as usize + BLOCK_CAPACITY >= ptr as usize + layout.size());

                return Some(ptr);
            }

            if let Some((cursor, limit)) = self
                .meta
                .find_next_available_hole(self.limit, layout.size())
            {
                self.cursor = cursor;
                self.limit = limit;
            } else {
                return None;
            }
        }
    }

    pub fn current_hole_size(&self) -> usize {
        self.cursor - self.limit
    }

    pub fn is_marked(&self, mark: NonZero<u8>) -> bool {
        self.meta.get_block_mark() == mark.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_block() {
        let mut b = BumpBlock::new().unwrap();

        for i in 0..BLOCK_CAPACITY {
            assert_eq!(b.cursor, BLOCK_CAPACITY - i);
            assert_eq!(b.current_hole_size(), BLOCK_CAPACITY - i);

            b.inner_alloc(Layout::new::<u8>()).unwrap();
        }

        assert!(b.inner_alloc(Layout::new::<u8>()).is_none());
    }

    #[test]
    fn test_current_hole_size() {
        let block = BumpBlock::new().unwrap();
        let expect = block.current_hole_size();

        assert_eq!(expect, BLOCK_CAPACITY);
    }
}

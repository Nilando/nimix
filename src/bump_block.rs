use super::block::Block;
use super::constants::{BLOCK_CAPACITY, SMALL_OBJECT_MIN};
use super::error::AllocError;
use alloc::alloc::Layout;
use core::num::NonZero;
use alloc::boxed::Box;

pub struct BumpBlock {
    cursor: usize,
    limit: usize,
    block: Box<Block>
}

impl BumpBlock {
    pub fn new() -> Result<BumpBlock, AllocError> {
        let block = Block::alloc()?;
        let bump_block = BumpBlock {
            cursor: BLOCK_CAPACITY,
            limit: 0,
            block,
        };

        Ok(bump_block)
    }

    pub fn reset_hole(&mut self, mark: NonZero<u8>) {
        self.block.free_unmarked(mark);

        if self.block.get_mark() != mark.into() {
            self.cursor = BLOCK_CAPACITY;
            self.limit = 0;
            return;
        }

        if let Some((cursor, limit)) = self.block
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

                let ptr = self.block.get_data_idx(self.cursor) as *const u8;

                return Some(ptr);
            }

            if let Some((cursor, limit)) = self.block
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
        self.block.get_mark() == mark.into()
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

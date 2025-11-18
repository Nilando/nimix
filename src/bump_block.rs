use super::block::Block;
use super::constants::{BLOCK_CAPACITY, SMALL_OBJECT_MIN};
use super::error::AllocError;
use alloc::alloc::Layout;
use core::num::NonZero;

pub struct BumpBlock {
    cursor: usize,
    limit: usize,
    block: Block
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

        if self.block.get_mark() != u8::from(mark) {
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

    pub fn inner_alloc(&mut self, layout: Layout) -> Option<*mut u8> {
        let size = layout.size();

        loop {
            // First, subtract the size to get the potential start position
            let potential_start = self.cursor.checked_sub(size)?;

            // Get the absolute address this would correspond to
            let potential_ptr = self.block.get_data_ptr(potential_start);
            let potential_addr = potential_ptr as usize;

            // Align the absolute address downward
            let aligned_addr = potential_addr & !(layout.align() - 1);

            // Calculate the offset back into our block's coordinate system
            let addr_adjustment = potential_addr - aligned_addr;
            let next = potential_start.checked_sub(addr_adjustment)?;

            if self.limit <= next {
                self.cursor = next;

                let ptr = self.block.get_data_ptr(self.cursor);

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
        self.block.get_mark() == u8::from(mark)
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

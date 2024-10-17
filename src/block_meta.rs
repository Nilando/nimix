use super::constants::{
    BLOCK_SIZE, FREE_MARK, LINE_COUNT, LINE_MARK_START, LINE_SIZE, BLOCK_MARK_OFFSET
};
use super::size_class::SizeClass;
use super::block::Block;
use std::sync::atomic::{AtomicU8, Ordering};
use std::num::NonZero;

pub struct BlockMeta {
    lines: *const [AtomicU8; LINE_COUNT],
    block_mark: *const AtomicU8,
}

impl BlockMeta {
    pub fn new(block: &Block) -> BlockMeta {
        let meta = unsafe { Self::from_block_ptr(block.as_ptr()) };

        meta.reset();
        meta
    }

    pub unsafe fn from_block_ptr(ptr: *const u8) -> Self {
        let lines = ptr.add(LINE_MARK_START) as *const [AtomicU8; LINE_COUNT];
        let block_mark =  ptr.add(BLOCK_MARK_OFFSET) as *const AtomicU8;

        Self {
            lines,
            block_mark,
        }
    }

    pub unsafe fn from_ptr(ptr: *const u8) -> Self {
        let offset = (ptr as usize) % BLOCK_SIZE;
        let block_ptr = ptr.byte_sub(offset);

        Self::from_block_ptr(block_ptr)
    }

    // SAFETY: ptr must be a point to an object allocated within a bump block
    pub unsafe fn mark(&self, ptr: *mut u8, size: u32, size_class: SizeClass, mark: NonZero<u8>) {
        let addr = ptr as usize;
        let relative_ptr = addr % BLOCK_SIZE;
        let line = relative_ptr / LINE_SIZE;

        debug_assert!(size_class != SizeClass::Large);

        if size_class == SizeClass::Small {
            self.set_line(line, mark.into());
        } else {
            let relative_end = relative_ptr + size as usize;
            let end_line = relative_end / LINE_SIZE;

            for i in line..end_line {
                self.set_line(i, mark.into());
            }
        }

        self.mark_block(mark);
    }

    pub fn free_unmarked(&self, mark: NonZero<u8>) {
        if self.get_block_mark() != mark.into() {
            self.free_block();
        }

        for i in 0..LINE_COUNT {
            if self.get_line(i) != mark.into() {
                self.set_line(i, FREE_MARK);
            }
        }
    }

    pub fn get_block_mark(&self) -> u8 {
        unsafe { (&*self.block_mark).load(Ordering::Relaxed) }
    }

    pub fn mark_block(&self, mark: NonZero<u8>) {
        unsafe { (&*self.block_mark).store(mark.into(), Ordering::Relaxed) }
    }

    pub fn reset(&self) {
        self.free_block();

        for i in 0..LINE_COUNT {
            self.set_line(i, FREE_MARK);
        }
    }

    pub fn find_next_available_hole(
        &self,
        starting_at: usize,
        alloc_size: usize,
    ) -> Option<(usize, usize)> {
        let mut free_line_count = 0;
        let starting_line = starting_at / LINE_SIZE;
        let lines_required = alloc_size.div_ceil(LINE_SIZE);
        let mut end = starting_line;

        for index in (0..starting_line).rev() {
            let line_mark = self.get_line(index);

            if line_mark == FREE_MARK {
                free_line_count += 1;

                if index == 0 && free_line_count >= lines_required {
                    let limit = index * LINE_SIZE;
                    let cursor = end * LINE_SIZE;

                    return Some((cursor, limit));
                }
            } else {
                if free_line_count > lines_required {
                    let limit = (index + 2) * LINE_SIZE;
                    let cursor = end * LINE_SIZE;

                    return Some((cursor, limit));
                }

                free_line_count = 0;
                end = index;
            }
        }

        None
    }

    fn get_line(&self, index: usize) -> u8 {
        self.mark_at(index).load(Ordering::Relaxed).into()
    }

    fn set_line(&self, index: usize, mark: u8) {
        self.mark_at(index).store(mark.into(), Ordering::Relaxed)
    }

    fn mark_at(&self, line: usize) -> &AtomicU8 {
        unsafe { &(&*self.lines)[line] }
    }

    fn free_block(&self) {
        unsafe { (&*self.block_mark).store(FREE_MARK, Ordering::Relaxed) }
    }
}

#[cfg(test)]
mod tests {
    use crate::constants::BLOCK_CAPACITY;
    use crate::block::Block;

    use super::*;
    use std::num::NonZero;

    #[test]
    fn new_block_meta_is_reset() {
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        assert_eq!(meta.get_block_mark(), FREE_MARK);

        for i in 0..LINE_COUNT {
            assert_eq!(meta.get_line(i), FREE_MARK);
        }
    }

    #[test]
    fn mark_block() {
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        meta.mark_block(NonZero::new(1).unwrap());

        assert_eq!(meta.get_block_mark(), 1);

        for i in 0..LINE_COUNT {
            assert_eq!(meta.get_line(i), FREE_MARK);
        }
    }

    #[test]
    fn mark_line() {
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        for i in 0..LINE_COUNT {
            let mark = 69;
            meta.set_line(i, mark);

            assert_eq!(mark, meta.get_line(i));
        }
    }

    #[test]
    fn find_next_hole() {
        // A set of marked lines with a couple holes.
        // The first hole should be seen as conservatively marked.
        // The second hole should be the one selected.
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        meta.set_line(9, 1);
        meta.set_line(10, 1);

        // line 5 should be conservatively marked
        let expect = Some((9 * LINE_SIZE, 0));

        let got = meta.find_next_available_hole(10 * LINE_SIZE, LINE_SIZE);

        assert_eq!(got, expect);
    }

    #[test]
    fn find_next_hole_at_line_zero() {
        // Should find the hole starting at the beginning of the block
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        meta.set_line(3, 1);

        let expect = Some((3 * LINE_SIZE, 0));

        let got = meta.find_next_available_hole(3 * LINE_SIZE, LINE_SIZE);

        assert_eq!(got, expect);
    }

    #[test]
    fn hole_with_conservatively_marked_line() {
        // hole size should reflect there being one line conservatively marked
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        meta.set_line(0, 1);
        meta.set_line(3, 1);

        // LIMIT is LINE_SIZE * 2, b/c line 0 is marked, therefore conservatively
        // marking line 1. Making the hole is be constrained to only line 2.
        let expect = Some((3 * LINE_SIZE, LINE_SIZE * 2));

        let got = meta.find_next_available_hole(3 * LINE_SIZE, LINE_SIZE);

        assert_eq!(got, expect);
    }

    #[test]
    fn find_next_hole_at_block_end() {
        // The first half of the block is marked.
        // The second half of the block should be identified as a hole.
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);
        let halfway = LINE_COUNT / 2;

        for i in halfway..LINE_COUNT {
            meta.set_line(i, 1);
        }

        // because halfway line should be conservatively marked
        let expect = Some((halfway * LINE_SIZE, 0));
        let got = meta.find_next_available_hole(BLOCK_CAPACITY, LINE_SIZE);

        assert_eq!(got, expect);
    }

    #[test]
    fn all_holes_conservatively_marked() {
        // Every other line is marked.
        // No hole should be found due to conservative marking.
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        for i in (0..LINE_COUNT).step_by(2) {
            meta.set_line(i, 1);
        }

        let got = meta.find_next_available_hole(BLOCK_CAPACITY, 1);
        assert_eq!(got, None);
    }

    #[test]
    fn entire_block_is_hole() {
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);
        let expect = (BLOCK_CAPACITY, 0);
        let got = meta.find_next_available_hole(BLOCK_CAPACITY, LINE_SIZE).unwrap();

        assert_eq!(got, expect);
    }

    #[test]
    fn reset_block_meta() {
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        meta.mark_block(NonZero::new(1).unwrap());

        for i in 0..LINE_COUNT {
            let mark = 69;
            meta.set_line(i, mark);
        }

        meta.reset();

        assert_eq!(meta.get_block_mark(), FREE_MARK);

        for i in 0..LINE_COUNT {
            assert_eq!(meta.get_line(i), FREE_MARK);
        }
    }
}

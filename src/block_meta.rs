use super::constants::{
    BLOCK_SIZE, FREE_MARK, LINE_COUNT, LINE_MARK_START, LINE_SIZE,
};
use super::size_class::SizeClass;
use super::block::Block;
use std::sync::atomic::{AtomicU8, Ordering};
use std::num::NonZero;

pub struct BlockMeta {
    lines: *const [AtomicU8; LINE_COUNT],
}

impl BlockMeta {
    pub fn new(block: &Block) -> BlockMeta {
        let meta = unsafe { Self::from_block_ptr(block.as_ptr()) };
        meta.reset();
        meta
    }

    pub unsafe fn from_block_ptr(ptr: *const u8) -> Self {
        Self {
            lines: ptr.add(LINE_MARK_START) as *const [AtomicU8; LINE_COUNT],
        }
    }

    pub unsafe fn from_ptr(ptr: *const u8) -> Self {
        let offset = (ptr as usize) % BLOCK_SIZE;

        unsafe {
            let block_ptr = ptr.byte_sub(offset);

            Self::from_block_ptr(block_ptr)
        }
    }

    pub unsafe fn mark(&self, ptr: *mut u8, size: u32, size_class: SizeClass, mark: NonZero<u8>) {
        let addr = ptr as usize;
        let relative_ptr = addr % BLOCK_SIZE;
        let line = relative_ptr / LINE_SIZE;

        debug_assert!(size_class != SizeClass::Large);

        if size_class == SizeClass::Small {
            self.set_line(line, mark.into());
        } else {
            let num_lines = size as u16 / LINE_SIZE as u16;

            for i in 0..num_lines {
                self.set_line(line + i as usize, mark.into());
            }
        }

        self.set_block(mark);
    }

    pub fn free_unmarked(&self, mark: NonZero<u8>) {
        for i in 0..LINE_COUNT {
            if self.get_line(i) != mark.into() {
                self.set_line(i, FREE_MARK);
            }
        }
    }

    pub fn get_block(&self) -> u8 {
        self.get_line(LINE_COUNT - 1)
    }

    fn get_line(&self, index: usize) -> u8 {
        self.mark_at(index).load(Ordering::Relaxed).into()
    }

    pub fn set_line(&self, index: usize, mark: u8) {
        self.mark_at(index).store(mark.into(), Ordering::Relaxed)
    }

    fn mark_at(&self, line: usize) -> &AtomicU8 {
        debug_assert!(line < LINE_COUNT);

        unsafe { &(&*self.lines)[line] }
    }

    pub fn set_block(&self, mark: NonZero<u8>) {
        self.set_line(LINE_COUNT - 1, mark.into())
    }

    pub fn reset(&self) {
        for i in 0..LINE_COUNT {
            self.set_line(i, FREE_MARK);
        }
    }

    pub fn find_next_available_hole(
        &self,
        starting_at: usize,
        alloc_size: usize,
    ) -> Option<(usize, usize)> {
        let mut count = 0;
        let starting_line = starting_at / LINE_SIZE;
        let lines_required = (alloc_size + LINE_SIZE - 1) / LINE_SIZE;
        let mut end = starting_line;

        for index in (0..starting_line).rev() {
            let line_mark = self.get_line(index);

            if line_mark == FREE_MARK {
                count += 1;

                if index == 0 && count >= lines_required {
                    let limit = index * LINE_SIZE;
                    let cursor = end * LINE_SIZE;

                    return Some((cursor, limit));
                }
            } else {
                if count > lines_required {
                    let limit = (index + 2) * LINE_SIZE;
                    let cursor = end * LINE_SIZE;

                    return Some((cursor, limit));
                }

                count = 0;
                end = index;
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::constants::BLOCK_CAPACITY;
    use super::super::block::Block;
    use super::*;
    use std::num::NonZero;
    use std::alloc::Layout;
    use crate::mark;

    #[test]
    fn mark_block() {
        // A set of marked lines with a couple holes.
        // The first hole should be seen as conservatively marked.
        // The second hole should be the one selected.
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        meta.set_block(NonZero::new(1).unwrap());
        let got = meta.get_block();

        assert_eq!(got, 1);
    }

    #[test]
    fn test_mark_line() {
        // A set of marked lines with a couple holes.
        // The first hole should be seen as conservatively marked.
        // The second hole should be the one selected.
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        meta.set_line(0, 1);

        let expect = 1;
        let got = meta.get_line(0);

        assert_eq!(got, expect);
    }

    #[test]
    fn test_find_next_hole() {
        // A set of marked lines with a couple holes.
        // The first hole should be seen as conservatively marked.
        // The second hole should be the one selected.
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        meta.set_line(0, 1);
        meta.set_line(1, 1);
        meta.set_line(2, 1);
        meta.set_line(4, 1);
        meta.set_line(10, 1);

        // line 5 should be conservatively marked
        let expect = Some((10 * LINE_SIZE, 6 * LINE_SIZE));

        let got = meta.find_next_available_hole(10 * LINE_SIZE, LINE_SIZE);

        assert_eq!(got, expect);
    }

    #[test]
    fn test_find_next_hole_at_line_zero() {
        // Should find the hole starting at the beginning of the block
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        meta.set_line(3, 1);
        meta.set_line(4, 1);
        meta.set_line(5, 1);

        let expect = Some((3 * LINE_SIZE, 0));

        let got = meta.find_next_available_hole(3 * LINE_SIZE, LINE_SIZE);

        assert_eq!(got, expect);
    }

    #[test]
    fn test_find_next_hole_at_block_end() {
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
    fn test_find_hole_all_conservatively_marked() {
        // Every other line is marked.
        // No hole should be found.
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);

        for i in 0..LINE_COUNT {
            if i % 2 == 0 {
                // there is no stable step function for range
                meta.set_line(i, 1);
            }
        }

        let got = meta.find_next_available_hole(BLOCK_CAPACITY, LINE_SIZE);
        assert_eq!(got, None);
    }

    #[test]
    fn test_find_entire_block() {
        // No marked lines. Entire block is available.
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);
        let expect = Some((BLOCK_CAPACITY, 0));
        let got = meta.find_next_available_hole(BLOCK_CAPACITY, LINE_SIZE);

        assert_eq!(got, expect);
    }

    #[test]
    fn mark_block_from_ptr() {
        let block = Block::default().unwrap();
        let meta = BlockMeta::new(&block);
        let medium = Layout::new::<[u8; 512]>();

        unsafe {
            let ptr: *const u8 = block.as_ptr();

            mark(ptr as *mut u8, medium, NonZero::new(1).unwrap()).expect("should mark");
        }

        assert_eq!(meta.get_block(), 1);
    }
}

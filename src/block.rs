use crate::constants::{BLOCK_CAPACITY, BLOCK_SIZE, FREE_MARK, LINE_COUNT, LINE_SIZE, META_CAPACITY};
use crate::size_class::SizeClass;

use super::error::AllocError;
use alloc::alloc::{alloc, Layout};
use core::num::NonZero;
use core::sync::atomic::{AtomicU8, Ordering};
use alloc::boxed::Box;

#[repr(C)]
pub struct Block {
    mark: AtomicU8,
    lines: [AtomicU8; LINE_COUNT],
    data: [u8; BLOCK_CAPACITY]
}

impl Block {
    pub fn alloc() -> Result<Box<Block>, AllocError> {
        unsafe {
            let layout = Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).unwrap();

            let ptr = alloc(layout);

            if ptr.is_null() {
                return Err(AllocError::OOM);
            }

            let box_block = Box::from_raw(ptr as *mut Block);

            box_block.reset();

            Ok(box_block)
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

                    debug_assert!(cursor > limit);

                    return Some((cursor, limit));
                }
            } else {
                if free_line_count > lines_required {
                    let limit = (index + 2) * LINE_SIZE;
                    let cursor = end * LINE_SIZE;

                    debug_assert!(cursor > limit);

                    return Some((cursor, limit));
                }

                free_line_count = 0;
                end = index;
            }
        }

        None
    }

    unsafe fn from_ptr<'a>(ptr: *const u8) -> &'a Block {
        let offset = (ptr as usize) % BLOCK_SIZE;
        let block_ptr = ptr.byte_sub(offset);

        &*(block_ptr as *const _)
    }

    pub unsafe fn mark(ptr: *const u8, layout: Layout, size_class: SizeClass, mark: NonZero<u8>) {
        let block = Block::from_ptr(ptr);
        let idx = (ptr as usize % BLOCK_SIZE) - META_CAPACITY;
        let line = idx / LINE_SIZE;

        if size_class == SizeClass::Small {
            block.set_line(line, mark.into());
        } else {
            let size = layout.size();
            let relative_end = (idx + size as usize) - 1;
            let end_line = relative_end / LINE_SIZE;

            for i in line..end_line {
                block.set_line(i, mark.into());
            }
        }

        block.mark_block(mark);
    }

    pub fn free_unmarked(&self, mark: NonZero<u8>) {
        if self.get_mark() != mark.into() {
            self.free_block();
        }

        for i in 0..LINE_COUNT {
            if self.get_line(i) != mark.into() {
                self.set_line(i, FREE_MARK);
            }
        }
    }

    fn reset(&self) {
        self.free_block();

        for i in 0..LINE_COUNT {
            self.set_line(i, FREE_MARK);
        }
    }

    pub fn get_data_idx(&self, idx: usize) -> &u8 {
        &self.data[idx]
    }

    pub fn get_mark(&self) -> u8 {
        self.mark.load(Ordering::Relaxed)
    }

    fn set_line(&self, line: usize, mark: u8) {
        self.lines[line].store(mark.into(), Ordering::Relaxed)
    }

    fn free_block(&self) {
        self.mark.store(FREE_MARK, Ordering::Relaxed)
    }

    fn get_line(&self, index: usize) -> u8 {
        self.lines[index].load(Ordering::Relaxed).into()
    }

    fn mark_block(&self, mark: NonZero<u8>) {
        self.mark.store(mark.into(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use crate::constants::BLOCK_SIZE;

    use super::*;

    #[test]
    fn size_of_block() {
        assert_eq!(core::mem::size_of::<Block>(), BLOCK_SIZE);
    }

    #[test]
    fn new_block_is_reset() {
        let block = Block::alloc().unwrap();

        assert_eq!(block.get_mark(), FREE_MARK);

        for i in 0..LINE_COUNT {
            assert_eq!(block.get_line(i), FREE_MARK);
        }
    }

    #[test]
    fn mark_block() {
        let block = Block::alloc().unwrap();

        block.mark_block(NonZero::new(1).unwrap());

        assert_eq!(block.get_mark(), 1);

        for i in 0..LINE_COUNT {
            assert_eq!(block.get_line(i), FREE_MARK);
        }
    }

    #[test]
    fn mark_line() {
        let block = Block::alloc().unwrap();

        for i in 0..LINE_COUNT {
            let mark = i;
            block.set_line(i, mark as u8);

            assert_eq!(i, block.get_line(i) as usize);
        }
    }

        #[test]
    fn find_next_hole() {
        // A set of marked lines with a couple holes.
        // The first hole should be seen as conservatively marked.
        // The second hole should be the one selected.
        let block = Block::alloc().unwrap();

        block.set_line(9, 1);
        block.set_line(10, 1);

        // line 5 should be conservatively marked
        let expect = Some((9 * LINE_SIZE, 0));

        let got = block.find_next_available_hole(10 * LINE_SIZE, LINE_SIZE);

        assert_eq!(got, expect);
    }

        #[test]
    fn find_next_hole_at_line_zero() {
        // Should find the hole starting at the beginning of the block
        let block = Block::alloc().unwrap();

        block.set_line(3, 1);

        let expect = Some((3 * LINE_SIZE, 0));

        let got = block.find_next_available_hole(3 * LINE_SIZE, LINE_SIZE);

        assert_eq!(got, expect);
    }

        #[test]
    fn hole_with_conservatively_marked_line() {
        // hole size should reflect there being one line conservatively marked
        let block = Block::alloc().unwrap();

        block.set_line(0, 1);
        block.set_line(3, 1);

        // LIMIT is LINE_SIZE * 2, b/c line 0 is marked, therefore conservatively
        // marking line 1. Making the hole is be constrained to only line 2.
        let expect = Some((3 * LINE_SIZE, LINE_SIZE * 2));

        let got = block.find_next_available_hole(3 * LINE_SIZE, LINE_SIZE);

        assert_eq!(got, expect);
    }
        #[test]
    fn find_next_hole_at_block_end() {
        // The first half of the block is marked.
        // The second half of the block should be identified as a hole.
        let block = Block::alloc().unwrap();
        let halfway = LINE_COUNT / 2;

        for i in halfway..LINE_COUNT {
            block.set_line(i, 1);
        }

        // because halfway line should be conservatively marked
        let expect = Some((halfway * LINE_SIZE, 0));
        let got = block.find_next_available_hole(BLOCK_CAPACITY, LINE_SIZE);

        assert_eq!(got, expect);
    }
        #[test]
    fn all_holes_conservatively_marked() {
        // Every other line is marked.
        // No hole should be found due to conservative marking.
        let block = Block::alloc().unwrap();

        for i in (0..LINE_COUNT).step_by(2) {
            block.set_line(i, 1);
        }

        let got = block.find_next_available_hole(BLOCK_CAPACITY, 1);
        assert_eq!(got, None);
    }

    #[test]
    fn entire_block_is_hole() {
        let block = Block::alloc().unwrap();
        let expect = (BLOCK_CAPACITY, 0);
        let got = block.find_next_available_hole(BLOCK_CAPACITY, LINE_SIZE).unwrap();

        assert_eq!(got, expect);
    }

    #[test]
    fn reset_block_block() {
        let block = Block::alloc().unwrap();

        block.mark_block(NonZero::new(1).unwrap());

        for i in 0..LINE_COUNT {
            let mark = 69;
            block.set_line(i, mark);
        }

        block.reset();

        assert_eq!(block.get_mark(), FREE_MARK);

        for i in 0..LINE_COUNT {
            assert_eq!(block.get_line(i), FREE_MARK);
        }
    }
}

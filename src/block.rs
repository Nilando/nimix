use crate::constants::{BLOCK_CAPACITY, BLOCK_SIZE, FREE_MARK, LINE_COUNT, LINE_SIZE, META_CAPACITY};
use crate::size_class::SizeClass;

use super::error::AllocError;
use alloc::alloc::{alloc, Layout};
use core::mem::ManuallyDrop;
use core::num::NonZero;
use core::sync::atomic::{AtomicU8, Ordering};

pub struct Block {
    mark: *mut AtomicU8,
    lines: *mut AtomicU8,
    data: *mut u8,
}

impl Block {
    pub fn alloc() -> Result<Block, AllocError> {
        unsafe {
            let layout = Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).unwrap();

            let ptr: *const u8 = alloc(layout);

            if ptr.is_null() {
                return Err(AllocError::OOM);
            }

            // Set up pointers to different regions within the single allocation
            let mark_ptr = ptr as *mut AtomicU8;
            let lines_ptr = ptr.add(core::mem::size_of::<AtomicU8>()) as *mut AtomicU8;
            let data_ptr = ptr.add(META_CAPACITY) as *mut u8;

            // Create the Block struct on the heap
            let block = Block {
                mark: mark_ptr,
                lines: lines_ptr,
                data: data_ptr,
            };

            block.reset();

            Ok(block)
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

    unsafe fn from_ptr(ptr: *const u8) -> ManuallyDrop<Block> {
        // Find the start of the block allocation by aligning down to BLOCK_SIZE
        let offset = (ptr as usize) % BLOCK_SIZE;
        let base_ptr = ptr.byte_sub(offset);

        // Reconstruct the Block struct from the base pointer
        // The memory layout is: mark (1 byte) | lines (LINE_COUNT bytes) | data (BLOCK_CAPACITY bytes)
        let mark_ptr = base_ptr as *mut AtomicU8;
        let lines_ptr = base_ptr.add(core::mem::size_of::<AtomicU8>()) as *mut AtomicU8;
        let data_ptr = base_ptr.add(META_CAPACITY) as *mut u8;

        // Create a Block on the heap and leak it to get a static reference
        // This is safe because the Block struct doesn't own the memory - it just points to it
        // The actual memory is managed separately through Block::alloc and Block::drop
        ManuallyDrop::new(Block {
            mark: mark_ptr,
            lines: lines_ptr,
            data: data_ptr,
        })
    }

    pub unsafe fn mark(ptr: *const u8, layout: Layout, size_class: SizeClass, mark: NonZero<u8>) {
        let block = Block::from_ptr(ptr);
        let idx = (ptr as usize % BLOCK_SIZE) - META_CAPACITY;
        let line = idx / LINE_SIZE;

        if size_class == SizeClass::Small {
            block.set_line(line, mark.into());
        } else {
            let size = layout.size();
            let relative_end = (idx + size) - 1;
            let end_line = relative_end / LINE_SIZE;

            for i in line..end_line {
                block.set_line(i, mark.into());
            }
        }

        block.mark_block(mark);
    }

    pub fn free_unmarked(&self, mark: NonZero<u8>) {
        if self.get_mark() != u8::from(mark) {
            self.free_block();
        }

        for i in 0..LINE_COUNT {
            if self.get_line(i) != u8::from(mark) {
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

    pub fn get_data_ptr(&self, idx: usize) -> *mut u8 {
        assert!(idx < BLOCK_CAPACITY,
            "get_data_ptr: allocation idx {} exceeds capacity {}",
            idx, BLOCK_CAPACITY);

        // Debug: validate data pointer is non-null
        debug_assert!(!self.data.is_null(), "get_data_ptr: data pointer is null");

        unsafe {
            let ptr = self.data.add(idx);

            // Debug: validate the computed pointer is reasonable
            debug_assert!(!ptr.is_null(), "get_data_ptr: computed pointer is null");
            debug_assert_eq!(
                (ptr as usize).wrapping_sub(self.data as usize),
                idx,
                "get_data_ptr: pointer arithmetic mismatch"
            );

            ptr
        }
    }

    pub fn get_mark(&self) -> u8 {
        unsafe {
            (*self.mark).load(Ordering::Relaxed)
        }
    }

    fn set_line(&self, line: usize, mark: u8) {
        unsafe {
            (*self.lines.add(line)).store(mark, Ordering::Relaxed)
        }
    }

    fn free_block(&self) {
        unsafe {
            (*self.mark).store(FREE_MARK, Ordering::Relaxed)
        }
    }

    fn get_line(&self, index: usize) -> u8 {
        unsafe {
            (*self.lines.add(index)).load(Ordering::Relaxed)
        }
    }

    fn mark_block(&self, mark: NonZero<u8>) {
        unsafe {
            (*self.mark).store(mark.into(), Ordering::Relaxed);
        }
    }
}

unsafe impl Send for Block {}
unsafe impl Sync for Block {}

impl Drop for Block {
    fn drop(&mut self) {
        unsafe {
            let layout = Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).unwrap();
            alloc::alloc::dealloc(self.mark as *mut u8, layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::constants::BLOCK_CAPACITY;

    use super::*;

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

    #[test]
    fn from_ptr_retrieves_correct_block() {
        // This test verifies that from_ptr can correctly reconstruct a block reference
        // from any pointer within the block's data region
        let block = Block::alloc().unwrap();

        // Mark the block with a unique mark
        let test_mark = NonZero::new(42).unwrap();
        block.mark_block(test_mark);

        // Get pointers at different offsets within the data region
        let ptr_at_start = block.get_data_ptr(0);
        let ptr_at_middle = block.get_data_ptr(BLOCK_CAPACITY / 2);
        let ptr_at_near_end = block.get_data_ptr(BLOCK_CAPACITY - 1);

        unsafe {
            // from_ptr should be able to reconstruct the block from any of these pointers
            let block_from_start = Block::from_ptr(ptr_at_start);
            let block_from_middle = Block::from_ptr(ptr_at_middle);
            let block_from_end = Block::from_ptr(ptr_at_near_end);

            // All reconstructed blocks should have the same mark we set
            assert_eq!(block_from_start.get_mark(), 42, "from_ptr failed for pointer at start");
            assert_eq!(block_from_middle.get_mark(), 42, "from_ptr failed for pointer at middle");
            assert_eq!(block_from_end.get_mark(), 42, "from_ptr failed for pointer at end");
        }
    }

    #[test]
    fn from_ptr_marks_correctly() {
        // This test verifies that marking through from_ptr works correctly
        let block = Block::alloc().unwrap();

        // Allocate at a specific offset
        let offset = LINE_SIZE * 5;
        let ptr = block.get_data_ptr(offset);
        let layout = Layout::from_size_align(LINE_SIZE, 8).unwrap();
        let mark = NonZero::new(7).unwrap();

        unsafe {
            // Mark the allocation using the static mark function
            Block::mark(ptr, layout, SizeClass::Small, mark);
        }

        // Verify the line was marked correctly
        let line = offset / LINE_SIZE;
        assert_eq!(block.get_line(line), 7, "Line was not marked correctly");
        assert_eq!(block.get_mark(), 7, "Block was not marked correctly");
    }
}

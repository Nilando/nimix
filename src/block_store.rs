use super::bump_block::BumpBlock;
use super::error::AllocError;
use super::constants::{BLOCK_SIZE, MAX_FREE_BLOCKS, RECYCLE_HOLE_MIN, LARGE_OBJECT_MIN};
use super::large_block::LargeBlock;
use super::atomic_stack::AtomicStack;
use alloc::alloc::Layout;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::num::NonZero;
use alloc::vec;

unsafe impl Send for BlockStore {}
unsafe impl Sync for BlockStore {}

pub struct BlockStore {
    block_count: AtomicUsize,
    rest: AtomicStack<BumpBlock>,
    large: AtomicStack<LargeBlock>,
    recycle: AtomicStack<BumpBlock>,
    free: AtomicStack<BumpBlock>,
}

impl BlockStore {
    pub fn new() -> Self {
        Self {
            block_count: AtomicUsize::new(0),
            free: AtomicStack::new(),
            recycle: AtomicStack::new(),
            rest: AtomicStack::new(),
            large: AtomicStack::new(),
        }
    }

    pub fn get_size(&self) -> usize {
        let block_space = self.block_count() * BLOCK_SIZE;
        let large_space = self.count_large_space();

        block_space + large_space
    }

    pub fn rest(&self, block: BumpBlock) {
        self.rest.push(block);
    }

    pub fn recycle(&self, block: BumpBlock) {
        if block.current_hole_size() >= RECYCLE_HOLE_MIN {
            self.recycle.push(block);
        } else {
            self.rest(block);
        }
    }

    pub fn get_head(&self) -> Result<BumpBlock, AllocError> {
        if let Some(recycle_block) = self.recycle.pop() {
            Ok(recycle_block)
        } else {
            self.get_overflow()
        }
    }

    pub fn get_overflow(&self) -> Result<BumpBlock, AllocError> {
        if let Some(free_block) = self.free.pop() {
            Ok(free_block)
        } else {
            self.new_block()
        }
    }

    pub fn block_count(&self) -> usize {
        self.block_count.load(Ordering::Relaxed)
    }

    pub fn count_large_space(&self) -> usize {
        // TODO: This is inefficient - drains and restores all items
        // Consider tracking size atomically or using a traversable lock-free list
        let items = self.large.drain_to_vec();
        let total = items.iter().fold(0, |sum, block| sum + block.get_size());
        self.large.push_from_iter(items);
        total
    }

    // large objects are stored with a single byte of meta info to store their mark
    pub fn create_large(&self, layout: Layout) -> Result<*const u8, AllocError> {
        assert!(layout.size() >= LARGE_OBJECT_MIN);

        let large_block = LargeBlock::new(layout)?;
        let ptr = large_block.as_ptr();

        self.large.push(large_block);

        Ok(ptr)
    }

    pub fn sweep(&self, mark: NonZero<u8>) {
        // Drain all stacks to process during sweep
        let large_items = self.large.drain_to_vec();
        let recycle_items = self.recycle.drain_to_vec();
        let rest_items = self.rest.drain_to_vec();

        let mut new_rest = vec![];
        let mut new_recycle = vec![];
        let mut new_large = vec![];
        let mut new_free = vec![];

        // Process large blocks
        for large_block in large_items {
            if large_block.is_marked(mark) {
                new_large.push(large_block);
            }
            // Unmarked blocks are dropped
        }

        // Process recycle blocks
        for mut block in recycle_items {
            block.reset_hole(mark);

            if block.is_marked(mark) {
                new_recycle.push(block);
            } else {
                new_free.push(block);
            }
        }

        // Process rest blocks
        for mut block in rest_items {
            block.reset_hole(mark);

            if block.is_marked(mark) {
                if block.current_hole_size() >= RECYCLE_HOLE_MIN {
                    new_recycle.push(block);
                } else {
                    new_rest.push(block);
                }
            } else {
                new_free.push(block);
            }
        }

        // Push everything back
        self.large.push_from_iter(new_large);
        self.recycle.push_from_iter(new_recycle);
        self.rest.push_from_iter(new_rest);

        // Only keep MAX_FREE_BLOCKS in the free list
        let mut kept_count = 0;
        for free_block in new_free.into_iter() {
            if kept_count < MAX_FREE_BLOCKS {
                self.free.push(free_block);
                kept_count += 1;
            } else {
                // Block is dropped, decrement count
                self.block_count.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }

    fn new_block(&self) -> Result<BumpBlock, AllocError> {
        self.block_count.fetch_add(1, Ordering::Relaxed);
        BumpBlock::new()
    }
}

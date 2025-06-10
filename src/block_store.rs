use super::bump_block::BumpBlock;
use super::error::AllocError;
use super::constants::{BLOCK_SIZE, MAX_FREE_BLOCKS, RECYCLE_HOLE_MIN, LARGE_OBJECT_MIN};
use super::large_block::LargeBlock;
use std::alloc::Layout;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::num::NonZero;

pub struct BlockStore {
    block_count: AtomicUsize,

    // TODO use channels instead of mutexes
    rest: Mutex<Vec<BumpBlock>>,
    large: Mutex<Vec<LargeBlock>>,
    recycle: Mutex<Vec<BumpBlock>>,
    free: Mutex<Vec<BumpBlock>>,
}

impl BlockStore {
    pub fn new() -> Self {
        Self {
            block_count: AtomicUsize::new(0),
            free: Mutex::new(vec![]),
            recycle: Mutex::new(vec![]),
            rest: Mutex::new(vec![]),
            large: Mutex::new(vec![]),
        }
    }

    pub fn get_size(&self) -> usize {
        let block_space = self.block_count() * BLOCK_SIZE;
        let large_space = self.count_large_space();

        block_space + large_space
    }

    pub fn rest(&self, block: BumpBlock) {
        self.rest.lock().unwrap().push(block);
    }

    pub fn recycle(&self, block: BumpBlock) {
        if block.current_hole_size() >= RECYCLE_HOLE_MIN {
            self.recycle.lock().unwrap().push(block);
        } else {
            self.rest(block);
        }
    }

    pub fn get_head(&self) -> Result<BumpBlock, AllocError> {
        if let Some(recycle_block) = self.recycle.lock().unwrap().pop() {
            Ok(recycle_block)
        } else {
            self.get_overflow()
        }
    }

    pub fn get_overflow(&self) -> Result<BumpBlock, AllocError> {
        if let Some(free_block) = self.free.lock().unwrap().pop() {
            Ok(free_block)
        } else {
            self.new_block()
        }
    }

    pub fn block_count(&self) -> usize {
        self.block_count.load(Ordering::Relaxed)
    }

    pub fn count_large_space(&self) -> usize {
        self.large
            .lock()
            .unwrap()
            .iter()
            .fold(0, |sum, block| sum + block.get_size())
    }

    // large objects are stored with a single byte of meta info to store their mark
    pub fn create_large(&self, layout: Layout) -> Result<*const u8, AllocError> {
        assert!(layout.size() >= LARGE_OBJECT_MIN);

        let large_block = LargeBlock::new(layout)?;
        let ptr = large_block.as_ptr();

        self.large.lock().unwrap().push(large_block);

        Ok(ptr)
    }

    // REFACTOR THIS: there needs to be a better story behind what this callback is
    pub fn sweep<F>(&self, mark: NonZero<u8>, sweep_callback: F) 
    where
        F: FnOnce()
    {
        let mut rest = self.rest.lock().unwrap();
        let mut large = self.large.lock().unwrap();
        let mut recycle = self.recycle.lock().unwrap();

        sweep_callback();

        let mut new_rest = vec![];
        let mut new_recycle = vec![];
        let mut new_large = vec![];
        let mut new_free = vec![];

        while let Some(large_block) = large.pop() {
            if large_block.is_marked(mark) {
                new_large.push(large_block);
            }
        }

        *large = new_large;
        drop(large);

        while let Some(mut block) = recycle.pop() {
            block.reset_hole(mark);

            if block.is_marked(mark) {
                new_recycle.push(block);
            } else {
                new_free.push(block);
            }
        }

        while let Some(mut block) = rest.pop() {
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

        *rest = new_rest;
        *recycle = new_recycle;
        drop(rest);
        drop(recycle);

        let mut free = self.free.lock().unwrap();
        while free.len() < MAX_FREE_BLOCKS && !new_free.is_empty() {
            let free_block = new_free.pop().unwrap();

            free.push(free_block);
        }

        self.block_count.fetch_sub(new_free.len(), Ordering::Relaxed);
    }

    fn new_block(&self) -> Result<BumpBlock, AllocError> {
        self.block_count.fetch_add(1, Ordering::Relaxed);
        BumpBlock::new()
    }
}

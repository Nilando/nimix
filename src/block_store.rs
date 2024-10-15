use super::bump_block::BumpBlock;
use super::error::AllocError;
use super::constants::{BLOCK_SIZE, MAX_FREE_BLOCKS, RECYCLE_HOLE_MIN};
use super::large_block::LargeBlock;
use std::alloc::Layout;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::num::NonZero;
use std::sync::LazyLock;

unsafe impl Send for BlockStore {}
unsafe impl Sync for BlockStore {}

pub static BLOCK_STORE: LazyLock<BlockStore> = LazyLock::new(|| BlockStore::new());

pub struct BlockStore {
    block_count: AtomicUsize,
    free: Mutex<Vec<BumpBlock>>,
    recycle: Mutex<Vec<BumpBlock>>,
    rest: Mutex<Vec<BumpBlock>>,
    large: Mutex<Vec<LargeBlock>>,
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

    pub fn push_rest(&self, block: BumpBlock) {
        self.rest.lock().unwrap().push(block);
    }

    pub fn push_recycle(&self, block: BumpBlock) {
        self.recycle.lock().unwrap().push(block);
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

        while let Some(block) = large.pop() {
            if block.is_marked(mark) {
                new_large.push(block);
            }
        }

        *large = new_large;
        drop(large);

        let mut free = self.free.lock().unwrap();
        while let Some(free_block) = new_free.pop() {
            if free.len() < MAX_FREE_BLOCKS {
                free.push(free_block);
            } else {
                break;
            }
        }

        self.block_count.fetch_sub(new_free.len(), Ordering::SeqCst);
    }

    fn new_block(&self) -> Result<BumpBlock, AllocError> {
        self.block_count.fetch_add(1, Ordering::SeqCst);
        BumpBlock::new()
    }
}

use super::block::Block;
use super::error::AllocError;
use super::constants::LARGE_OBJECT_MIN;

use std::ptr::write;
use std::alloc::Layout;
use std::num::NonZero;
use std::sync::atomic::{AtomicU8, Ordering};

pub struct LargeBlock {
    block: Block,
    mark: *const AtomicU8
}

impl LargeBlock {
    pub fn new(obj_layout: Layout) -> Result<Self, AllocError> {
        debug_assert!(obj_layout.size() >= LARGE_OBJECT_MIN);

        let mark_layout = Layout::new::<AtomicU8>();
        let (obj_mark_layout, mark_offset) = obj_layout.extend(mark_layout)?;
        let block = Block::new(obj_mark_layout.pad_to_align())?;
        let mark = unsafe { block.as_ptr().add(mark_offset) } as *const AtomicU8;
        let large_block = Self {
            block,
            mark
        };

        Ok(large_block)
    }

    pub unsafe fn mark(ptr: *const u8, obj_layout: Layout, mark: NonZero<u8>) -> Result<(), AllocError> {
        let mark_layout = Layout::new::<AtomicU8>();
        let (_, mark_offset) = obj_layout.extend(mark_layout)?;
        let block_mark: *const AtomicU8 = ptr.add(mark_offset) as *const AtomicU8;

        (&*block_mark).store(mark.into(), Ordering::Relaxed);

        Ok(())
    }

    pub fn is_marked(&self, mark: NonZero<u8>) -> bool {
        unsafe { (&*self.mark).load(Ordering::Relaxed) == mark.into() }
    }

    pub fn get_size(&self) -> usize {
        self.block.get_size()
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.block.as_ptr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_large_block() {
        let align = 8;
        let block =  LargeBlock::new(Layout::from_size_align(LARGE_OBJECT_MIN, align).unwrap()).unwrap();

        assert!(block.get_size() > LARGE_OBJECT_MIN);
        assert_eq!(block.as_ptr() as usize % align, 0);
        assert_eq!(block.is_marked(NonZero::new(1).unwrap()), false);
    }

    #[test]
    fn mark_large() {
        let data = [0u8; LARGE_OBJECT_MIN];
        let align = 8;
        let layout = Layout::from_size_align(LARGE_OBJECT_MIN, align).unwrap();
        let block =  LargeBlock::new(layout).unwrap();

        unsafe { 
            write(block.as_ptr() as *mut [u8; LARGE_OBJECT_MIN], data);
            LargeBlock::mark(block.as_ptr(), layout, NonZero::new(1).unwrap()).unwrap();

            assert_eq!(&*(block.as_ptr() as *mut [u8; LARGE_OBJECT_MIN]), &data);
        }

        assert!(block.is_marked(NonZero::new(1).unwrap()));
    }
}

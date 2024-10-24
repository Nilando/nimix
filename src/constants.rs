pub const FREE_MARK: u8 = 0;
pub const BLOCK_SIZE: usize = 1024 * 16;
pub const LINE_SIZE: usize = 128;
pub const LINE_COUNT: usize = (BLOCK_SIZE - 1) / (LINE_SIZE + 1);
pub const BLOCK_CAPACITY: usize = LINE_COUNT * LINE_SIZE;
pub const LINE_MARK_START: usize = BLOCK_CAPACITY;
pub const BLOCK_MARK_OFFSET: usize = LINE_MARK_START + LINE_COUNT;
pub const MAX_ALLOC_SIZE: usize = u32::MAX as usize;
pub const SMALL_OBJECT_MIN: usize = 1;
pub const SMALL_OBJECT_MAX: usize = LINE_SIZE;
pub const MEDIUM_OBJECT_MIN: usize = SMALL_OBJECT_MAX + 1;
pub const MEDIUM_OBJECT_MAX: usize = BLOCK_CAPACITY;
pub const LARGE_OBJECT_MIN: usize = MEDIUM_OBJECT_MAX + 1;
pub const LARGE_OBJECT_MAX: usize = MAX_ALLOC_SIZE;
pub const MAX_FREE_BLOCKS: usize = 100;
pub const RECYCLE_HOLE_MIN: usize = LINE_SIZE * 5;

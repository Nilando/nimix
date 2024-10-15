pub const FREE_MARK: u8 = 0;
pub const BLOCK_SIZE: usize = 1024 * 32;
pub const LINE_SIZE: usize = 128;

// How many total lines are in a block, but one of these lines is actually just
// for line mark bits. This is pretty confusing..probably desrves a rename.
// Like RAW_LINES, and LINE_COUNT can be something else
pub const LINE_COUNT: usize = BLOCK_SIZE / LINE_SIZE;

pub const BLOCK_CAPACITY: usize = BLOCK_SIZE - LINE_COUNT;
pub const LINE_MARK_START: usize = BLOCK_CAPACITY;
pub const MAX_ALLOC_SIZE: usize = u32::MAX as usize;
pub const SMALL_OBJECT_MIN: usize = 1;
pub const SMALL_OBJECT_MAX: usize = LINE_SIZE;
pub const MEDIUM_OBJECT_MIN: usize = SMALL_OBJECT_MAX + 1;
pub const MEDIUM_OBJECT_MAX: usize = BLOCK_CAPACITY;
pub const LARGE_OBJECT_MIN: usize = MEDIUM_OBJECT_MAX + 1;
pub const LARGE_OBJECT_MAX: usize = MAX_ALLOC_SIZE;
pub const MAX_FREE_BLOCKS: usize = 100;
pub const RECYCLE_HOLE_MIN: usize = LINE_SIZE * 5;

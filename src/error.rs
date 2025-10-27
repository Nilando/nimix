use alloc::alloc::LayoutError;

impl From<LayoutError> for AllocError {
    fn from(_: LayoutError) -> Self {
        Self::LayoutError
    }
}

#[derive(Debug)]
pub enum AllocError {
    OOM,
    AllocOverflow,
    LayoutError,
}

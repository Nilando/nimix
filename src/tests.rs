use super::allocator::Allocator;
use super::constants::{BLOCK_CAPACITY, BLOCK_SIZE};
use std::alloc::Layout;
use std::num::NonZero;

#[test]
fn hello_alloc() {
    let a = Allocator::new();
    let name = "Hello Alloc";
    let layout = Layout::for_value(&name);

    assert_eq!(a.get_size(), 0);

    unsafe { a.alloc(layout).unwrap(); }

    assert_eq!(a.get_size(), BLOCK_SIZE);
}

#[test]
fn alloc_large() {
    let a = Allocator::new();
    let data: [usize; BLOCK_SIZE] = [0; BLOCK_SIZE];
    let layout = Layout::for_value(&data);

    assert_eq!(a.get_size(), 0);

    unsafe { a.alloc(layout).unwrap(); }
}

#[test]
fn alloc_many_single_bytes() {
    let a = Allocator::new();
    let layout = Layout::new::<u8>();

    for _ in 0..100_000 {
        unsafe { a.alloc(layout).unwrap(); }
    }
}

#[test]
fn alloc_too_big() {
    let a = Allocator::new();
    let layout = Layout::from_size_align(std::u32::MAX as usize + 1 as usize, 8).unwrap();
    let result = unsafe { a.alloc(layout) };
    assert!(result.is_err());
}

#[test]
fn alloc_two_large_arrays() {
    let a = Allocator::new();
    let layout = Layout::from_size_align((BLOCK_CAPACITY / 2) + 1, 8).unwrap();

    unsafe { a.alloc(layout).unwrap(); }
    assert_eq!(a.get_size(), BLOCK_SIZE);
    unsafe { a.alloc(layout).unwrap(); }
    assert_eq!(a.get_size(), BLOCK_SIZE * 2);
}

#[test]
fn refresh_arena() {
    let a = Allocator::new();
    let layout = Layout::from_size_align((BLOCK_CAPACITY / 2) + 1, 8).unwrap();

    for _ in 0..2000 {
        unsafe { a.alloc(layout).unwrap(); }
    }

    assert!(a.get_size() > 10 * BLOCK_SIZE);
    unsafe { a.sweep(NonZero::new(1).unwrap(), || {}); }
    assert_eq!(a.get_size(), BLOCK_SIZE);
}

#[test]
fn object_align() {
    let a = Allocator::new();
    for i in 0..10 {
        let align: usize = 2_usize.pow(i);
        let layout = Layout::from_size_align(32, align).unwrap();
        let ptr = unsafe { a.alloc(layout).unwrap() };

        assert!((ptr as usize % align) == 0);
    }
}

#[test]
fn large_object_align() {
    let a = Allocator::new();
    let layout = Layout::from_size_align(BLOCK_CAPACITY * 2, 128).unwrap();
    let ptr = unsafe { a.alloc(layout).unwrap() };

    assert!((ptr as usize % 128) == 0)
}

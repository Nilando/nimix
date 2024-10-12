use std::alloc::Layout;
use std::num::NonZero;
use nimix::{
    alloc,
    sweep,
};

#[test]
fn hello_alloc() {
    let name = "Hello Alloc";
    let layout = Layout::for_value(&name);

    unsafe { alloc(layout).unwrap(); }
}

#[test]
fn alloc_large() {
    let data: [u8; 100_000] = [0; 100_000];
    let layout = Layout::for_value(&data);

    for _ in 0..10 {
        unsafe { alloc(layout).unwrap(); }
    }
}

#[test]
fn alloc_many_single_bytes() {
    let layout = Layout::new::<u8>();

    for _ in 0..100_000 {
        unsafe { alloc(layout).unwrap(); }
    }
}

#[test]
fn alloc_too_big() {
    let layout = Layout::from_size_align(std::u32::MAX as usize + 1 as usize, 8).unwrap();
    let result = unsafe { alloc(layout) };

    assert!(result.is_err());
}


#[test]
fn refresh_arena() {
    let layout = Layout::from_size_align(64, 8).unwrap();

    unsafe {
        for _ in 0..2000 {
            alloc(layout).unwrap();
        }

        sweep(NonZero::new(1).unwrap(), || {}); 
    }
}

#[test]
fn object_align() {
    for i in 0..10 {
        let align: usize = 2_usize.pow(i);
        let layout = Layout::from_size_align(32, align).unwrap();
        let ptr = unsafe { alloc(layout).unwrap() };

        assert!((ptr as usize % align) == 0);
    }
}

#[test]
fn large_object_align() {
    let layout = Layout::from_size_align(1024* 1024, 128).unwrap();
    let ptr = unsafe { alloc(layout).unwrap() };

    assert!((ptr as usize % 128) == 0)
}

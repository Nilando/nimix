// The fuzzer bascially keeps an in memory copy of all values
//
// And does multi threaded mutation of the graph
use rand::prelude::*;
use std::collections::HashMap;
use std::alloc::Layout;
use std::num::NonZero;
use nimix::{
    sweep,
    mark,
    alloc,
    get_size,
};

unsafe impl Send for Fuzzer {}
unsafe impl Sync for Fuzzer {}

#[derive(Clone)]
struct Value {
    data: Vec<u8>,
}

impl Value {
    fn new(size: usize) -> Self {
        let mut data = Vec::with_capacity(size);
        let mut rng = rand::thread_rng();

        for _ in 0..size {
            let n: u8 = rng.gen();

            data.push(n);
        }

        Self {
            data
        }
    }
}

#[derive(Clone)]
struct Fuzzer {
    values: HashMap<*const u8, Value>,
    marker: NonZero<u8>,
}

impl Fuzzer {
    fn new(marker: NonZero<u8>) -> Self {
        Self {
            values: HashMap::new(),
            marker
        }
    }

    fn assert(&self) {
        for (ptr, value) in self.values.iter() {
            for (i, v) in value.data.iter().enumerate() {
                unsafe { assert!(*ptr.add(i) == *v) }
            }
        }
    }

    fn alloc(&mut self) {
        let mut rng = rand::thread_rng();

        for _ in 0..ALLOC_LOOPS {
            let size = rng.gen_range(1..=1024 * 5);
            let value = Value::new(size);

            unsafe {
                let power = rng.gen_range(0..=8);
                let align = 2usize.pow(power);
                let layout = Layout::from_size_align_unchecked(size, align);
                let dest = alloc(layout).unwrap();

                for _ in 0..size {
                    let src = value.data.as_ptr();
                    std::ptr::copy_nonoverlapping(src, dest, size);
                }

                let coin_flip = rng.gen_range(0..100);
                if coin_flip < 5 {
                    self.values.insert(dest, value);
                    mark(dest, layout, self.marker).unwrap();
                } else {
                    // this is garbage and will be swept
                }
            }
        }
    }
}

const NUM_THREADS: usize = 16;
const MARK_LOOPS: usize = 10;
const SWEEP_LOOPS: usize = 10;
const ALLOC_LOOPS: usize = 200;

#[test]
fn fuzz() {
    for l in 1..=MARK_LOOPS {
        println!("=== MARK LOOP {l} ===");

        let marker = NonZero::new(l as u8).unwrap();
        let mut fuzzers = vec![];
        for _ in 0..NUM_THREADS {
            fuzzers.push(Fuzzer::new(marker));
        }

        for _ in 0..SWEEP_LOOPS {
            let mut join_handles = vec![];

            for _ in 0..NUM_THREADS {
                let mut fuzzer = fuzzers.pop().unwrap();

                fuzzer.assert();
                
                let jh = std::thread::spawn(move || {
                    fuzzer.alloc();
                    fuzzer
                });

                join_handles.push(jh);
            }

            // sweep while fuzzers are marking
            unsafe { sweep(marker, || {}); }

            for jh in join_handles.into_iter() {
                fuzzers.push(jh.join().unwrap());
            }
        }

        let bytes: f64 = get_size() as f64;
        let mb = (bytes / 1024.0) / 1024.0;
        println!("HEAP SIZE: {:.2} mb", mb);
    }
}

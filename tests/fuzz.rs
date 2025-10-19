// The fuzzer bascially keeps an in memory copy of all values
//
// And does multi threaded mutation of the graph
use rand::prelude::*;
use std::collections::HashMap;
use std::alloc::Layout;
use std::num::NonZero;
use nimix::Heap;

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
    heap: Heap,
    values: HashMap<*const u8, Value>,
    marker: NonZero<u8>,
}

impl Fuzzer {
    fn new(heap: Heap, marker: NonZero<u8>) -> Self {
        Self {
            heap,
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
            let mut size = rng.gen_range(1..=2_000);

            if size == 2_000 {
                size = 1024 * 17;
            }

            let value = Value::new(size);

            unsafe {
                let power = rng.gen_range(0..=8);
                let align = 2usize.pow(power);
                let layout = Layout::from_size_align(size, align).unwrap();
                let dest = self.heap.alloc(layout).unwrap();

                for _ in 0..size {
                    let src = value.data.as_ptr();
                    std::ptr::copy_nonoverlapping(src, dest, size);
                }

                let coin_flip = rng.gen_range(0..1000);
                if coin_flip < 5 {
                    self.values.insert(dest, value);
                    Heap::mark(dest, layout, self.marker).unwrap();
                } else {
                    // this is garbage and will be swept
                }
            }
        }
    }
}

const NUM_THREADS: usize = 8;
const SWEEP_LOOPS: usize = 4;
const ALLOC_LOOPS: usize = 100;

#[test]
fn fuzz() {
    let heap = Heap::new();

    for l in 1..=3 {
        println!("=== MARK LOOP {l} ===");

        let marker = NonZero::new(l as u8).unwrap();
        let mut fuzzers = vec![];
        for _ in 0..NUM_THREADS {
            fuzzers.push(Fuzzer::new(heap.clone(), marker));
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

            for jh in join_handles.into_iter() {
                fuzzers.push(jh.join().unwrap());
            }

            unsafe { heap.sweep(marker, || {}); }
        }

        let bytes: f64 = heap.size() as f64;
        let mb = (bytes / 1024.0) / 1024.0;

        println!("HEAP SIZE: {:.2} mb", mb);
    }
}

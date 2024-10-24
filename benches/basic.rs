use criterion::{
    criterion_group, 
    criterion_main, 
    Criterion, 
    Throughput, 
    BenchmarkId
};

use nimix::Heap;
use std::alloc::Layout;
use std::num::NonZero;

fn alloc_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("alloc sizes");

    for size in [1, 2, 4, 8, 16, 32, 64, 128].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        let heap = Heap::new();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let layout = Layout::from_size_align(size, 1).unwrap();
            b.iter(|| unsafe { heap.alloc(layout) });
            unsafe { heap.sweep(NonZero::new(1u8).unwrap(), || {}) };
        });
    }

    group.finish();
}

criterion_group!(benches, alloc_sizes);
criterion_main!(benches);

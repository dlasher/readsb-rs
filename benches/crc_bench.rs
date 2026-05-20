use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_crc_correction(c: &mut Criterion) {
    c.bench_function("crc_correction", |b| b.iter(|| {
        black_box(42)
    }));
}

criterion_group!(benches, bench_crc_correction);
criterion_main!(benches);

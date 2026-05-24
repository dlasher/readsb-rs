use criterion::{criterion_group, criterion_main, Criterion};
use readsb::demod::{convert_to_magnitude, demodulate2400, InputFormat};

fn bench_convert_sc16q11(c: &mut Criterion) {
    let input: Vec<u8> = (0..480000).map(|i| (i % 256) as u8).collect();
    let mut output = vec![0u16; 240000];
    c.bench_function("convert_sc16q11", |b|
        b.iter(|| {
            convert_to_magnitude(&input, InputFormat::SC16Q11, &mut output)
        })
    );
}

fn bench_demodulate2400(c: &mut Criterion) {
    use readsb::demod::MagBufStats;
    let mag: Vec<u16> = vec![100u16; 240000];
    c.bench_function("demodulate2400_noise", |b|
        b.iter(|| {
            demodulate2400(&mag, mag.len(), 32768, &mut MagBufStats::default())
        })
    );
}

criterion_group!(benches, bench_convert_sc16q11, bench_demodulate2400);
criterion_main!(benches);

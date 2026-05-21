use criterion::{criterion_group, criterion_main, Criterion};
use readsb::crc::{modes_checksum, CrcFixEngine};

const MSG: [u8; 14] = [0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3, 0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98];

fn bench_modes_checksum(c: &mut Criterion) {
    c.bench_function("modes_checksum", |b| b.iter(|| {
        modes_checksum(&MSG, 112)
    }));
}

fn bench_crc_diagnose(c: &mut Criterion) {
    let engine = CrcFixEngine::new(112);
    // Corrupted message that produces a known single-bit error syndrome
    let mut msg = MSG;
    msg[5] ^= 0x01;
    let syndrome = modes_checksum(&msg, 112);
    c.bench_function("crc_diagnose", |b|
        b.iter(|| {
            engine.diagnose(syndrome)
        })
    );
}

criterion_group!(benches, bench_modes_checksum, bench_crc_diagnose);
criterion_main!(benches);

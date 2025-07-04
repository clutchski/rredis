use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rredis::parser::parse_simple_string;
use bytes::BytesMut;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("parse_simple_string", |b| {
        b.iter(|| {
            let mut buf = BytesMut::from(&b"+OK\r\n"[..]);
            parse_simple_string(black_box(&mut buf)).unwrap()
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
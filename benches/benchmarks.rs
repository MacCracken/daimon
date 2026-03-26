use criterion::{Criterion, criterion_group, criterion_main};

fn bench_config_default(c: &mut Criterion) {
    c.bench_function("config_default", |b| {
        b.iter(|| daimon::Config::default());
    });
}

criterion_group!(benches, bench_config_default);
criterion_main!(benches);

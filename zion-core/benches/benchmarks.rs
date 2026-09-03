use criterion::{black_box, criterion_group, criterion_main, Criterion};
use zion_core::monitoring::{Hypothalamus, SystemVitals};

fn bench_hypothalamus_evaluation(c: &mut Criterion) {
    c.bench_function("hypothalamus_evaluation", |b| {
        b.iter(|| {
            let mut hypothalamus = Hypothalamus::default();
            hypothalamus.evaluate(black_box(SystemVitals::new(86.0, 92.0, 120.0, 0.2)));
        })
    });
}

criterion_group!(benches, bench_hypothalamus_evaluation);
criterion_main!(benches);

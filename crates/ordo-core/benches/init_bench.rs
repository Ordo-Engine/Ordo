use criterion::{criterion_group, criterion_main, Criterion};
use ordo_core::prelude::*;
use std::hint::black_box;

fn bench_init(c: &mut Criterion) {
    c.bench_function("function_registry_new", |b| {
        b.iter(|| black_box(FunctionRegistry::new()))
    });

    c.bench_function("evaluator_new", |b| b.iter(|| black_box(Evaluator::new())));

    c.bench_function("rule_executor_new", |b| {
        b.iter(|| black_box(RuleExecutor::new()))
    });
}

criterion_group!(benches, bench_init);
criterion_main!(benches);

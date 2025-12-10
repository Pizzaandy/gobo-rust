use criterion::{Criterion, criterion_group, criterion_main};
use gobo_rust::lex;
use gobo_rust::source_text::SourceText;
use std::hint::black_box;

pub fn bench_lex(c: &mut Criterion) {
    static TEST_SOURCE: &str = include_str!("large_file.gml");
    let text = SourceText::from_str(TEST_SOURCE);

    c.bench_function("bench_lex", |b| {
        b.iter(|| {
            let result = lex::lex(black_box(&text));
        });
    });
}

criterion_group!(benches, bench_lex);
criterion_main!(benches);

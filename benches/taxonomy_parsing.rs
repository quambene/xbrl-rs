use criterion::{Criterion, criterion_group, criterion_main};
use std::{path::PathBuf, str::FromStr, time::Duration};
use xbrl_rs::TaxonomySet;

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";

fn bench_full_dts_2020(c: &mut Criterion) {
    // All 6 entry points as used in a real HGB instance document.
    let schema_refs =     vec![
        "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd".to_owned(),
        "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd"
            .to_owned(),
        "http://www.xbrl.de/taxonomies/de-bra-2020-04-01/de-bra-2020-04-01-shell-fiscal.xsd"
            .to_owned(),
        "http://www.xbrl.de/taxonomies/de-fi-2020-04-01/de-fi-2020-04-01-shell-staffelform-fiscal.xsd"
            .to_owned(),
        "http://www.xbrl.de/taxonomies/de-ins-2020-04-01/de-ins-2020-04-01-shell-fiscal.xsd"
            .to_owned(),
        "http://www.xbrl.de/taxonomies/de-pi-2020-04-01/de-pi-2020-04-01-shell-staffelform-fiscal.xsd"
            .to_owned(),
    ];
    let mut group = c.benchmark_group("taxonomy_discovery");

    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    group.warm_up_time(Duration::from_secs(5));
    group.bench_function("full_dts_2020", |b| {
        b.iter(|| {
            let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
            TaxonomySet::discover(schema_refs.clone(), entry_point).unwrap()
        });
    });
    group.finish();
}

fn bench_single_dts_2020(c: &mut Criterion) {
    // Single entry point benchmark.
    let schema_refs = vec![
        "http://www.xbrl.de/taxonomies/de-bra-2020-04-01/de-bra-2020-04-01-shell-fiscal.xsd"
            .to_owned(),
    ];

    let mut group = c.benchmark_group("taxonomy_discovery");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    group.warm_up_time(Duration::from_secs(5));
    group.bench_function("single_dts_2020", |b| {
        b.iter(|| {
            let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
            TaxonomySet::discover(schema_refs.clone(), entry_point).unwrap()
        });
    });
    group.finish();
}

criterion_group!(benches, bench_full_dts_2020, bench_single_dts_2020);
criterion_main!(benches);

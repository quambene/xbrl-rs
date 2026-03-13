use criterion::{Criterion, criterion_group, criterion_main};
use quick_xml::Reader;
use std::{path::PathBuf, str::FromStr, time::Duration};
use xbrl_rs::{InstanceDocument, TaxonomySet};

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";

fn parse_instance(c: &mut Criterion) {
    let mut group = c.benchmark_group("bench_xbrl");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(5));
    group.bench_function("parse_instance", |b| {
        b.iter(|| {
            let reader = Reader::from_file("test_data/instances/balance_sheet_v64.xml").unwrap();
            let _instance = InstanceDocument::from_xml(reader).unwrap();
        });
    });
}

fn validate_instance(c: &mut Criterion) {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(
                vec![
                    "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd".to_owned(),
                    "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd"
                        .to_owned(),
                ],
                entry_point,
            )
            .unwrap();
    let reader = Reader::from_file("test_data/instances/balance_sheet_v64.xml").unwrap();
    let instance = InstanceDocument::from_xml(reader).unwrap();

    let mut group = c.benchmark_group("bench_xbrl");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(5));
    group.bench_function("validate_instance", |b| {
        b.iter(|| instance.validate(&taxonomy));
    });
}

fn view_instance(c: &mut Criterion) {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(
                vec![
                    "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd".to_owned(),
                    "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd"
                        .to_owned(),
                ],
                entry_point,
            )
            .unwrap();
    let reader = Reader::from_file("test_data/instances/balance_sheet_v64.xml").unwrap();
    let instance = InstanceDocument::from_xml(reader).unwrap();

    let mut group = c.benchmark_group("bench_xbrl");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(5));
    group.bench_function("view_instance", |b| {
        b.iter(|| instance.validate(&taxonomy));
    });
}

fn bench_single_dts_2020(c: &mut Criterion) {
    // Single entry point benchmark.
    let schema_refs = vec![
        "http://www.xbrl.de/taxonomies/de-bra-2020-04-01/de-bra-2020-04-01-shell-fiscal.xsd"
            .to_owned(),
    ];

    let mut group = c.benchmark_group("bench_xbrl");
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

    let mut group = c.benchmark_group("bench_xbrl");
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

criterion_group!(
    benches,
    parse_instance,
    validate_instance,
    view_instance,
    bench_single_dts_2020,
    bench_full_dts_2020,
);
criterion_main!(benches);

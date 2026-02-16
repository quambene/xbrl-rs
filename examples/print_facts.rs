//! Print all facts of an XBRL instance as a grid in the terminal.
//!
//! Usage:
//!     cargo run --example print_facts -- test_data/instances/ebilanz/v6.4/HandelsbilanzGastronom_PersG.xml

use quick_xml::Reader;
use std::{fs::File, io::BufReader};
use xbrl_rs::XbrlInstance;

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}~", &s[..max - 1])
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        "test_data/instances/ebilanz/v6.4/HandelsbilanzGastronom_PersG.xml".into()
    });

    let file = File::open(&path)?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    let instance = XbrlInstance::from_xml(&mut reader)?;

    let facts: Vec<_> = instance
        .facts()
        .iter()
        .filter(|fact| !fact.is_nil())
        .collect();

    // Fixed column widths for better readability
    let w_concept = 100;
    let w_ctx = 16;
    let w_value = 20;
    let w_unit = 4;
    let w_dec = 3;

    println!(
        "{:<w_concept$}  {:<w_ctx$}  {:>w_value$}  {:>w_unit$}  {:>w_dec$}",
        "CONCEPT", "CONTEXT", "VALUE", "UNIT", "DEC",
    );
    println!(
        "{:-<w_concept$}  {:-<w_ctx$}  {:-<w_value$}  {:-<w_unit$}  {:-<w_dec$}",
        "", "", "", "", "",
    );

    for fact in &facts {
        println!(
            "{:<w_concept$}  {:<w_ctx$}  {:>w_value$}  {:>w_unit$}  {:>w_dec$}",
            truncate(fact.concept(), w_concept),
            truncate(fact.context_ref(), w_ctx),
            truncate(fact.value(), w_value),
            fact.unit_ref().unwrap_or(""),
            fact.decimals().unwrap_or(""),
        );
    }

    println!(
        "\n{} facts ({} nil hidden)",
        facts.len(),
        instance.facts().len() - facts.len()
    );

    Ok(())
}

use quick_xml::{Reader, Writer};
use std::{fs::File, io::BufReader, path::Path};
use xbrl_rs::XbrlInstance;

const INSTANCE_BASE: &str = "test_data/instances/ebilanz";

fn parse_instance(path: &Path) -> XbrlInstance {
    let file = File::open(path).expect("failed to open instance file");
    let mut reader = Reader::from_reader(BufReader::new(file));

    XbrlInstance::from_xml(&mut reader).expect("failed to parse instance")
}

#[test]
fn write_instance_v64_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_instance(&path);

    let mut writer: Writer<Vec<u8>> = Writer::new_with_indent(Vec::new(), b' ', 2);

    instance.to_xml(&mut writer).unwrap();
}

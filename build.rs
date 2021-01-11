use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::path::Path;

fn main() {
    let response = ureq::get("http://standards-oui.ieee.org/oui.txt")
        .call()
        .expect("Conection Error");
    let mut reader = BufReader::new(response.into_reader());

    let mut data: Vec<(Vec<u8>, String)> = Vec::new();
    let mut line = Vec::new();
    while reader.read_until(b'\n', &mut line).unwrap() != 0 {
        if line.get(12..=18).map_or(false, |s| s == b"base 16") {
            let mac_oui = line[0..6].to_owned();
            let vendor = String::from_utf8_lossy(&line[22..]).trim().to_owned();
            data.push((mac_oui, vendor));
        }
        line.clear();
    }
    data.sort_unstable();
    let mut logs = BufReader::new(File::open("../datasets/logs-conexion.csv").unwrap());
    let mut unique_macs = HashSet::new();
    while logs.read_until(b'\n', &mut line).unwrap() != 0 {
        let mac_oui = &line[11..17];
        unique_macs.insert(mac_oui.to_owned());
        line.clear();
    }
    data.retain(|(k, _)| unique_macs.contains(k));

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("map_oui.rs");
    let handle = File::create(dest_path).unwrap();
    let mut writer = BufWriter::new(handle);
    writeln!(
        &mut writer,
        "const MAP_MACS: [([u8; 6], &str); {}] = [",
        data.len()
    )
    .unwrap();
    for (key, value) in data {
        writeln!(&mut writer, "    ({:?}, \"{}\"),", key, value).unwrap();
    }
    writeln!(&mut writer, "];").unwrap();
    writer.flush().unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}

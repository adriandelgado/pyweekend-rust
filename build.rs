use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("map_oui.rs");
    let handle = File::create(dest_path).unwrap();
    let mut writer = BufWriter::new(handle);
    let response = ureq::get("http://standards-oui.ieee.org/oui.txt")
        .call()
        .expect("Error al conectarse a IEEE");
    let mut reader = BufReader::new(response.into_reader());
    let mut line = Vec::new();

    writer
        .write(
            b"#[rustfmt::skip]
lazy_static! {
    static ref MAP_MACS: FxHashMap<&'static [u8; 6], &'static str> = {
    let mut map_macs = HashMap::with_capacity_and_hasher(29231, CustomHasher::default());\n",
        )
        .unwrap();
    loop {
        match reader.read_until('\n' as u8, &mut line) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                if line.get(12..=18).map_or(false, |s| s == b"base 16") {
                    let mac_oui = String::from_utf8_lossy(&line[0..6]);
                    let fabricante = String::from_utf8_lossy(&line[22..]);
                    writer.write(b"    map_macs.insert(b\"").unwrap();
                    writer.write(mac_oui.as_bytes()).unwrap();
                    writer.write(b"\", \"").unwrap();
                    writer.write(fabricante.trim().as_bytes()).unwrap();
                    writer.write(b"\");\n").unwrap();
                }
                line.clear();
            }
            Err(_) => (),
        }
    }
    writer
        .write(
            b"    map_macs
    };
}
",
        )
        .unwrap();
    writer.flush().unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}

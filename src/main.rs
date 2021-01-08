use chrono::NaiveDateTime;
use plotters::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    hash::BuildHasherDefault,
    io::{self, prelude::*, BufReader},
    path::Path,
    time::Instant,
};
type CustomHasher = BuildHasherDefault<FxHasher>;
fn main() -> io::Result<()> {
    let dataset = Path::new("../datasets/logs-conexion.csv");
    let aps_espol = Path::new("../datasets/aps_espol.csv");
    let map_macs = crear_cache_macs();

    let now0 = Instant::now();
    // for (fabricante, cantidad) in fab_mas_comunes(dataset, &map_macs)? {
    //     println!("{:<40} {:>6}", fabricante, cantidad);
    // }
    fab_mas_comunes(dataset, &map_macs)?;
    println!("fab_mas_comunes: {} ms", now0.elapsed().as_millis());

    let now1 = Instant::now();
    generar_grafico(dataset, &map_macs)?;
    println!("generar_grafico: {} ms", now1.elapsed().as_millis());

    let now2 = Instant::now();
    total_bytes(dataset)?;
    println!("total_bytes: {} ms", now2.elapsed().as_millis());

    let mac_ap = "40A6E8:6C:5B:05";
    let timestamp = "1607173201";
    let now3 = Instant::now();
    clientes_unicos(dataset, mac_ap, timestamp)?;
    println!("clientes_unicos: {} ms", now3.elapsed().as_millis());

    let mac_cliente = "C46699:FD:B6:4E";
    let now4 = Instant::now();
    cambio_edificio(dataset, aps_espol, mac_cliente)?;
    println!("cambio_edificio: {} ms", now4.elapsed().as_millis());

    Ok(())
}

fn csv_lines<P: AsRef<Path>>(csv: P) -> impl Iterator<Item = Result<String, io::Error>> {
    let file_handle = File::open(csv).expect("No se pudo abrir el CSV");
    let reader = BufReader::new(file_handle);
    reader.lines().skip(1)
}

fn fab_mas_comunes<P: AsRef<Path>>(
    dataset: P,
    map_macs: &FxHashMap<&str, &str>,
) -> io::Result<Vec<(String, u32)>> {
    let mut counter: FxHashMap<&str, u32> =
        HashMap::with_capacity_and_hasher(77, CustomHasher::default());
    let mut dispositivos_previos = HashSet::with_capacity_and_hasher(1000, CustomHasher::default());
    let dt = File::open(dataset)?;
    let mut reader_dt = BufReader::new(dt);
    let mut line = String::new();
    reader_dt.read_line(&mut line)?;
    line.clear();
    loop {
        match reader_dt.read_line(&mut line) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                if dispositivos_previos.insert(line[11..26].to_owned()) {
                    let fabricante = map_macs
                        .get(&line[11..17])
                        .expect(&format!("mac rara: {}", &line[11..17]));
                    *counter.entry(fabricante).or_insert(0) += 1;
                }
                line.clear();
            }
            Err(err) => return Err(err),
        }
    }
    let mut result: Vec<(&str, u32)> = counter.into_iter().collect();
    result.sort_unstable_by_key(|(_, v)| std::cmp::Reverse(*v));
    result.truncate(10);
    Ok(result.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn generar_grafico<P: AsRef<Path>>(dataset: P, map_macs: &FxHashMap<&str, &str>) -> io::Result<()> {
    let (fabricantes, cantidades): (Vec<String>, Vec<u32>) = fab_mas_comunes(dataset, map_macs)?
        .into_iter()
        .rev()
        .unzip();
    let root = BitMapBackend::new("top10_fabricantes.jpg", (1433, 860)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Top 10 de fabricantes en dataset",
            ("sans-serif", 25, "bold").into_font(),
        )
        .margin(20)
        .margin_top(10)
        .x_label_area_size(20)
        .y_label_area_size(320)
        .build_cartesian_2d(0u32..1700u32, fabricantes.into_segmented())
        .unwrap();

    chart
        .configure_mesh()
        .y_label_formatter(&|x| -> String {
            match x {
                SegmentValue::Exact(v) => format!("{} ", v),
                SegmentValue::CenterOf(v) => format!("{} ", v),
                _ => "".to_string(),
            }
        })
        .x_label_style(("sans-serif", 16).into_font())
        .y_label_style(("sans-serif", 16).into_font())
        .draw()
        .unwrap();
    let br = chart.as_coord_spec().y_spec().to_owned();
    let data = (0..).zip(cantidades.into_iter());

    chart
        .draw_series(data.map(|(y, x)| {
            let (y, ny) = (br.from_index(y).unwrap(), br.from_index(y + 1).unwrap());
            let style = RGBColor(0, 100, 200).filled();
            let mut rect = Rectangle::new([(0, y), (x, ny)], style);
            rect.set_margin(10, 10, 0, 0);
            rect
        }))
        .unwrap();

    Ok(())
}

fn total_bytes<P: AsRef<Path>>(
    dataset: P,
) -> io::Result<FxHashMap<String, FxHashMap<String, FxHashMap<String, u32>>>> {
    let mut result: FxHashMap<String, FxHashMap<String, FxHashMap<String, u32>>> =
        HashMap::with_capacity_and_hasher(179974, CustomHasher::default());
    let dt = File::open(dataset)?;
    let mut reader_dt = BufReader::new(dt);
    let mut line = String::new();
    reader_dt.read_line(&mut line)?;
    line.clear();
    loop {
        match reader_dt.read_line(&mut line) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                let timestamp: i64 = line[..10].parse().unwrap();
                let fecha = NaiveDateTime::from_timestamp(timestamp, 0)
                    .date()
                    .to_string();
                let mac_ap = line[27..42].to_owned();
                let bts: u32 = line[43..49].parse().unwrap();
                if &line[50..56] == "upload" {
                    *result
                        .entry(fecha)
                        .or_default()
                        .entry(mac_ap)
                        .or_default()
                        .entry("recibidos".to_owned())
                        .or_insert(0) += bts;
                } else {
                    *result
                        .entry(fecha)
                        .or_default()
                        .entry(mac_ap)
                        .or_default()
                        .entry("enviados".to_owned())
                        .or_insert(0) += bts;
                }
                line.clear();
            }
            Err(err) => return Err(err),
        }
    }
    Ok(result)
}

fn clientes_unicos<P: AsRef<Path>>(
    dataset: P,
    mac_ap: &str,
    timestamp: &str,
) -> io::Result<Vec<String>> {
    let mut previas = FxHashSet::default();
    let mut result = Vec::new();
    let timestamp: i64 = timestamp.parse().unwrap();
    let dt = File::open(dataset)?;
    let mut reader_dt = BufReader::new(dt);
    let mut line = String::new();
    reader_dt.read_line(&mut line)?;
    line.clear();
    loop {
        match reader_dt.read_line(&mut line) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                if &line[27..42] == mac_ap
                    && &line[50..56] == "upload"
                    && previas.insert(line[11..26].to_owned())
                {
                    let current: i64 = line[..10].parse().unwrap();
                    if 0 <= current - timestamp && current - timestamp <= 10800 {
                        let mac_cliente = line[11..26].to_owned();
                        result.push(mac_cliente);
                    }
                }
                line.clear();
            }
            Err(err) => return Err(err),
        }
    }
    Ok(result)
}

fn cambio_edificio<P: AsRef<Path>>(
    dataset: P,
    aps_espol: P,
    mac_cliente: &str,
) -> io::Result<Vec<String>> {
    let mut map_aps = HashMap::with_capacity_and_hasher(394, CustomHasher::default());
    for line in csv_lines(aps_espol) {
        let line = line?;
        map_aps.insert(line[..15].to_owned(), line[16..18].to_owned());
    }
    let mut edificio_previo = String::new();
    let mut lista = Vec::new();
    let dt = File::open(dataset)?;
    let mut reader_dt = BufReader::new(dt);
    let mut line = String::new();
    reader_dt.read_line(&mut line)?;
    line.clear();
    loop {
        match reader_dt.read_line(&mut line) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                if &line[11..26] == mac_cliente {
                    let edificio_actual = map_aps.get(&line[27..42]).unwrap().as_str();
                    if edificio_previo != edificio_actual {
                        edificio_previo.replace_range(.., edificio_actual);
                        let timestamp: i64 = line[..10].parse().unwrap();
                        let fecha_hora = NaiveDateTime::from_timestamp(timestamp, 0)
                            .format("%Y-%b-%d %H:%M:%S")
                            .to_string();
                        lista.push(fecha_hora);
                    }
                }
                line.clear();
            }
            Err(err) => return Err(err),
        }
    }
    Ok(lista)
}

// fn crear_cache_macs() -> io::Result<HashMap<String, String>> {
//     let response = ureq::get("http://standards-oui.ieee.org/oui.txt")
//         .call()
//         .expect("Error al conectarse a IEEE");
//     let reader = BufReader::new(response.into_reader());
//     let mut map_macs = HashMap::with_capacity(29231);
//     for line in reader.lines() {
//         let line = line?;
//         if line.get(12..=18).map_or(false, |s| s == "base 16") {
//             map_macs.insert(line[..6].to_owned(), line[21..].trim_start().to_owned());
//         }
//     }
//     Ok(map_macs)
// }

#[rustfmt::skip]
fn crear_cache_macs() -> FxHashMap<&'static str, &'static str> {
    let mut map_macs = HashMap::with_capacity_and_hasher(1276, CustomHasher::default());
    map_macs.insert("00A081", "ALCATEL DATA NETWORKS");
    map_macs.insert("002060", "ALCATEL ITALIA S.p.A.");
    map_macs.insert("008039", "ALCATEL STC AUSTRALIA");
    map_macs.insert("002032", "ALCATEL TAISEL");
    map_macs.insert("00809F", "ALE International");
    map_macs.insert("00153F", "Alcatel Alenia Space Italia");
    map_macs.insert("000F62", "Alcatel Bell Space N.V.");
    map_macs.insert("00113F", "Alcatel DI");
    map_macs.insert("0CB5DE", "Alcatel Lucent");
    map_macs.insert("18422F", "Alcatel Lucent");
    map_macs.insert("4CA74B", "Alcatel Lucent");
    map_macs.insert("54055F", "Alcatel Lucent");
    map_macs.insert("68597F", "Alcatel Lucent");
    map_macs.insert("84A783", "Alcatel Lucent");
    map_macs.insert("885C47", "Alcatel Lucent");
    map_macs.insert("9067F3", "Alcatel Lucent");
    map_macs.insert("94AE61", "Alcatel Lucent");
    map_macs.insert("D4224E", "Alcatel Lucent");
    map_macs.insert("00089A", "Alcatel Microelectronics");
    map_macs.insert("000E86", "Alcatel North America");
    map_macs.insert("000502", "Apple, Inc.");
    map_macs.insert("00A040", "Apple, Inc.");
    map_macs.insert("080007", "Apple, Inc.");
    map_macs.insert("00040F", "Asus Network Technologies, Inc.");
    map_macs.insert("3CBD3E", "Beijing Xiaomi Electronics Co., Ltd.");
    map_macs.insert("08003E", "CODEX CORPORATION");
    map_macs.insert("1C48CE", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("1C77F6", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("2C5BB8", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("38295A", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("440444", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("4C1A3D", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("6C5C14", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("88D50C", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("8C0EE3", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("A09347", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("A43D78", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("A81B5A", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("B0AA36", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("B83765", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("BC3AEA", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("C09F05", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("C8F230", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("CC2D83", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("D4503F", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("DC6DCD", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("E44790", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("E8BBA8", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("EC01EE", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("ECF342", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert("00092D", "HTC Corporation");
    map_macs.insert("002376", "HTC Corporation");
    map_macs.insert("00EEBD", "HTC Corporation");
    map_macs.insert("04C23E", "HTC Corporation");
    map_macs.insert("188796", "HTC Corporation");
    map_macs.insert("1CB094", "HTC Corporation");
    map_macs.insert("2C8A72", "HTC Corporation");
    map_macs.insert("38E7D8", "HTC Corporation");
    map_macs.insert("404E36", "HTC Corporation");
    map_macs.insert("502E5C", "HTC Corporation");
    map_macs.insert("64A769", "HTC Corporation");
    map_macs.insert("7C6193", "HTC Corporation");
    map_macs.insert("800184", "HTC Corporation");
    map_macs.insert("807ABF", "HTC Corporation");
    map_macs.insert("847A88", "HTC Corporation");
    map_macs.insert("902155", "HTC Corporation");
    map_macs.insert("980D2E", "HTC Corporation");
    map_macs.insert("A0F450", "HTC Corporation");
    map_macs.insert("A826D9", "HTC Corporation");
    map_macs.insert("AC3743", "HTC Corporation");
    map_macs.insert("B4CEF6", "HTC Corporation");
    map_macs.insert("BCCFCC", "HTC Corporation");
    map_macs.insert("D40B1A", "HTC Corporation");
    map_macs.insert("D4206D", "HTC Corporation");
    map_macs.insert("D8B377", "HTC Corporation");
    map_macs.insert("E899C4", "HTC Corporation");
    map_macs.insert("F8DB7F", "HTC Corporation");
    map_macs.insert("001882", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("001E10", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("002568", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("00259E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("0034FE", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("00464B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("005A13", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("00664B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("009ACD", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("00E0FC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("00F81C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("04021F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("0425C5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("042758", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("043389", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("047503", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("049FCA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("04B0E7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("04BD70", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("04C06F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("04F938", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("04FE8D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("0819A6", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("086361", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("087A4C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("08C021", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("08E84F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("0C37DC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("0C45BA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("0C96BF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("0CD6BD", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("101B54", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("104780", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("105172", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("10B1F8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("10C61F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("143004", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("145F94", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("149D09", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("14A0F8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("14A51A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("14B968", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("14D11F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("18C58A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("18D276", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("18DED7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("1C1D67", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("1C6758", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("1C8E5C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("2008ED", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("200BC7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("202BC1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("20A680", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("20F17C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("20F3A3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("2400BA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("240995", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("241FA0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("244427", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("244C07", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("2469A5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("247F3C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("249EAB", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("24BCF8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("24DBAC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("24DF6A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("283152", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("283CE4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("285FDB", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("286ED4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("28B448", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("2C55D3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("2C9D1E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("2CAB00", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("307496", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("308730", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("30D17E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("30F335", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("3400A3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("341E6B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("346AC2", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("346BD3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("34A2A2", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("34B354", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("34CDBE", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("384C4F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("38BC01", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("38F889", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("3C4711", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("3C678C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("3CDFBD", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("3CF808", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("3CFA43", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("404D8E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("407D0F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("40CBA8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("4455B1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("446A2E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("446EE5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("4482E5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("44C346", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("480031", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("483C0C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("48435A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("486276", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("487B6B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("48AD08", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("48D539", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("48DB50", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("48FD8E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("4C1FCC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("4C5499", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("4C8BEF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("4CB16C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("4CF95D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("4CFB45", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("50016B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("5001D9", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("5004B8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("50680A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("509F27", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("50A72B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("5425EA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("5439DF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("54511B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("548998", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("54A51B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("581F28", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("582AF7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("58605F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("587F66", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("5C4CA9", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("5C7D5E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("5CA86A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("5CB395", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("5CB43E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("5CF96A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("600810", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("608334", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("60DE44", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("60E701", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("6416F0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("643E8C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("64A651", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("6889C1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("688F84", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("68A0F6", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("68A828", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("68CC6E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("7054F5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("70723C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("707990", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("707BE8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("708A09", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("70A8E3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("745AAA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("74882A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("749D8F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("74A063", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("74A528", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("781DBA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("786A89", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("78D752", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("78F557", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("78F5FD", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("7C11CB", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("7C1CF1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("7C6097", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("7C7D3D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("7CA23E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("7CB15D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("801382", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("8038BC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("80717A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("80B686", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("80D09B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("80D4A5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("80FB06", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("8421F1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("844765", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("845B12", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("849FB5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("84A8E4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("84A9C4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("84AD58", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("84BE52", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("84DBAC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("8828B3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("883FD3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("884477", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("8853D4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("886639", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("888603", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("88A2D7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("88CEFA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("88CF98", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("88E3AB", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("8C0D76", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("8C34FD", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("8CEBC6", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("900325", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("9017AC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("904E2B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("90671C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("94049C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("94772B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("94DBDA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("94FE22", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("98E7F5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("9C28EF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("9C37F4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("9C52F8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("9C741A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("9C7DA3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("9CB2B2", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("9CC172", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("9CE374", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A0086F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A08CF8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A08D16", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A0A33B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A0F479", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A47174", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A49947", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A4BA76", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A4C64F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A4CAA0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A4DCBE", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A8C83A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("A8CA7B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("AC4E91", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("AC6175", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("AC853D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("ACCF85", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("ACE215", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("ACE87B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("B05B67", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("B08900", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("B0E5ED", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("B41513", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("B43052", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("B808D7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("B8BC1B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("BC25E0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("BC3F8F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("BC620E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("BC7574", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("BC7670", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("BC9C31", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C07009", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C0BFC0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C40528", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C4072F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C4473F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C486E9", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C4F081", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C4FF1F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C80CC8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C81451", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C81FBE", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C85195", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C88D83", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C894BB", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("C8D15E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("CC53B5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("CC96A0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("CCA223", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("CCCC81", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D02DB3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D03E5C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D065CA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D06F82", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D07AB5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D0D04B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D0FF98", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D440F0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D4612E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D46AA8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D46E5C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D494E8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D4A148", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D4B110", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D4F9A1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D8490B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("D8C771", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("DC094C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("DCC64B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("DCD2FC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("DCD916", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("DCEE06", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E0191D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E0247F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E02861", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E03676", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E09796", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E0A3AC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E468A3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E47E66", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E4A8B6", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E4C2D1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E8088B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E84DD0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E8BDD1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("E8CD2D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("EC233D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("EC388F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("EC4D47", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("ECCB30", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F02FA7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F04347", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F09838", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F0C850", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F44C7F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F4559C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F48E92", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F49FF3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F4C714", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F4CB52", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F4DCF9", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F80113", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F823B2", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F83DFF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F84ABF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F87588", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F898B9", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F8BF09", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("F8E811", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("FC3F7C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("FC48EF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("FCE33C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert("0022A1", "Huawei Symantec Technologies Co.,Ltd.");
    map_macs.insert("B0989F", "LG CNS");
    map_macs.insert("F0182B", "LG Chem");
    map_macs.insert("3CE624", "LG Display");
    map_macs.insert("D84FB8", "LG ELECTRONICS");
    map_macs.insert("3CBDD8", "LG ELECTRONICS INC");
    map_macs.insert("3CCD93", "LG ELECTRONICS INC");
    map_macs.insert("9893CC", "LG ELECTRONICS INC");
    map_macs.insert("CC2D8C", "LG ELECTRONICS INC");
    map_macs.insert("E85B5B", "LG ELECTRONICS INC");
    map_macs.insert("00E091", "LG Electronics");
    map_macs.insert("14C913", "LG Electronics");
    map_macs.insert("388C50", "LG Electronics");
    map_macs.insert("6CD032", "LG Electronics");
    map_macs.insert("C808E9", "LG Electronics");
    map_macs.insert("001C62", "LG Electronics (Mobile Communications)");
    map_macs.insert("001E75", "LG Electronics (Mobile Communications)");
    map_macs.insert("001F6B", "LG Electronics (Mobile Communications)");
    map_macs.insert("001FE3", "LG Electronics (Mobile Communications)");
    map_macs.insert("0021FB", "LG Electronics (Mobile Communications)");
    map_macs.insert("0022A9", "LG Electronics (Mobile Communications)");
    map_macs.insert("002483", "LG Electronics (Mobile Communications)");
    map_macs.insert("0025E5", "LG Electronics (Mobile Communications)");
    map_macs.insert("0026E2", "LG Electronics (Mobile Communications)");
    map_macs.insert("00AA70", "LG Electronics (Mobile Communications)");
    map_macs.insert("0C4885", "LG Electronics (Mobile Communications)");
    map_macs.insert("10683F", "LG Electronics (Mobile Communications)");
    map_macs.insert("10F96F", "LG Electronics (Mobile Communications)");
    map_macs.insert("2021A5", "LG Electronics (Mobile Communications)");
    map_macs.insert("2C54CF", "LG Electronics (Mobile Communications)");
    map_macs.insert("2C598A", "LG Electronics (Mobile Communications)");
    map_macs.insert("30766F", "LG Electronics (Mobile Communications)");
    map_macs.insert("344DF7", "LG Electronics (Mobile Communications)");
    map_macs.insert("34FCEF", "LG Electronics (Mobile Communications)");
    map_macs.insert("40B0FA", "LG Electronics (Mobile Communications)");
    map_macs.insert("485929", "LG Electronics (Mobile Communications)");
    map_macs.insert("505527", "LG Electronics (Mobile Communications)");
    map_macs.insert("583F54", "LG Electronics (Mobile Communications)");
    map_macs.insert("58A2B5", "LG Electronics (Mobile Communications)");
    map_macs.insert("5C70A3", "LG Electronics (Mobile Communications)");
    map_macs.insert("5CAF06", "LG Electronics (Mobile Communications)");
    map_macs.insert("60E3AC", "LG Electronics (Mobile Communications)");
    map_macs.insert("64899A", "LG Electronics (Mobile Communications)");
    map_macs.insert("64BC0C", "LG Electronics (Mobile Communications)");
    map_macs.insert("6CD68A", "LG Electronics (Mobile Communications)");
    map_macs.insert("700514", "LG Electronics (Mobile Communications)");
    map_macs.insert("74A722", "LG Electronics (Mobile Communications)");
    map_macs.insert("78F882", "LG Electronics (Mobile Communications)");
    map_macs.insert("805A04", "LG Electronics (Mobile Communications)");
    map_macs.insert("88074B", "LG Electronics (Mobile Communications)");
    map_macs.insert("88C9D0", "LG Electronics (Mobile Communications)");
    map_macs.insert("8C3AE3", "LG Electronics (Mobile Communications)");
    map_macs.insert("98D6F7", "LG Electronics (Mobile Communications)");
    map_macs.insert("A039F7", "LG Electronics (Mobile Communications)");
    map_macs.insert("A09169", "LG Electronics (Mobile Communications)");
    map_macs.insert("A816B2", "LG Electronics (Mobile Communications)");
    map_macs.insert("A8922C", "LG Electronics (Mobile Communications)");
    map_macs.insert("A8B86E", "LG Electronics (Mobile Communications)");
    map_macs.insert("AC0D1B", "LG Electronics (Mobile Communications)");
    map_macs.insert("B81DAA", "LG Electronics (Mobile Communications)");
    map_macs.insert("BCF5AC", "LG Electronics (Mobile Communications)");
    map_macs.insert("C4438F", "LG Electronics (Mobile Communications)");
    map_macs.insert("C49A02", "LG Electronics (Mobile Communications)");
    map_macs.insert("CCFA00", "LG Electronics (Mobile Communications)");
    map_macs.insert("D013FD", "LG Electronics (Mobile Communications)");
    map_macs.insert("DC0B34", "LG Electronics (Mobile Communications)");
    map_macs.insert("E892A4", "LG Electronics (Mobile Communications)");
    map_macs.insert("F01C13", "LG Electronics (Mobile Communications)");
    map_macs.insert("F80CF3", "LG Electronics (Mobile Communications)");
    map_macs.insert("F8A9D0", "LG Electronics (Mobile Communications)");
    map_macs.insert("001256", "LG INFORMATION & COMM.");
    map_macs.insert("0019A1", "LG INFORMATION & COMM.");
    map_macs.insert("0050CE", "LG INTERNATIONAL CORP.");
    map_macs.insert("0005C9", "LG Innotek");
    map_macs.insert("001EB2", "LG Innotek");
    map_macs.insert("1C08C1", "LG Innotek");
    map_macs.insert("30A9DE", "LG Innotek");
    map_macs.insert("944444", "LG Innotek");
    map_macs.insert("A06FAA", "LG Innotek");
    map_macs.insert("C4366C", "LG Innotek");
    map_macs.insert("C80210", "LG Innotek");
    map_macs.insert("E8F2E2", "LG Innotek");
    map_macs.insert("A48CDB", "Lenovo");
    map_macs.insert("A03299", "Lenovo (Beijing) Co., Ltd.");
    map_macs.insert("207693", "Lenovo (Beijing) Limited.");
    map_macs.insert("74042B", "Lenovo Mobile Communication (Wuhan) Company Limited");
    map_macs.insert("E02CB2", "Lenovo Mobile Communication (Wuhan) Company Limited");
    map_macs.insert("1436C6", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("149FE8", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("503CC4", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("60D9A0", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("6C5F1C", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("70720D", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("80CF41", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("88708C", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("98FFD0", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("AC3870", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("C8DDC9", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("CC07E4", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("D87157", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("EC89F5", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert("00E00C", "MOTOROLA");
    map_macs.insert("002075", "MOTOROLA COMMUNICATION ISRAEL");
    map_macs.insert("000A28", "Motorola");
    map_macs.insert("4888CA", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert("542758", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert("64DB43", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert("7C4685", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert("980CA5", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert("D0F88C", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert("002437", "Motorola - BSG");
    map_macs.insert("482CEA", "Motorola Inc Business Light Radios");
    map_macs.insert("000EC7", "Motorola Korea");
    map_macs.insert("141AA3", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("1430C6", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("24DA9B", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("34BB26", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("40786A", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("408805", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("4480EB", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("5C5188", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("60BEB5", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("68C44D", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("8058F8", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("806C1B", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("84100D", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("88797E", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("9068C3", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("9CD917", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("A470D6", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("B07994", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("CC61E5", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("CCC3EA", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("E0757D", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("E09861", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("E4907E", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("E89120", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("EC8892", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("F0D7AA", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("F4F1E1", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("F4F524", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("F8CFC5", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("F8E079", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("F8F1B6", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert("4CCC34", "Motorola Solutions Inc.");
    map_macs.insert("0080D6", "NUVOTECH, INC.");
    map_macs.insert("00198F", "Nokia Bell N.V.");
    map_macs.insert("0002EE", "Nokia Danmark A/S");
    map_macs.insert("000EED", "Nokia Danmark A/S");
    map_macs.insert("000BE1", "Nokia NET Product Operations");
    map_macs.insert("000FBB", "Nokia Siemens Networks GmbH & Co. KG.");
    map_macs.insert("00061B", "Notebook Development Lab.  Lenovo Japan Ltd.");
    map_macs.insert("0022DE", "OPPO Digital, Inc.");
    map_macs.insert("C0EEFB", "OnePlus Tech (Shenzhen) Ltd");
    map_macs.insert("94652D", "OnePlus Technology (Shenzhen) Co., Ltd");
    map_macs.insert("000278", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("002119", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("002637", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("206432", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("38AA3C", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("50CCF8", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("5C0A5B", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("78D6F0", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("840B2D", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("90187C", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("980C82", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("A00BBA", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("B407F9", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("CC3A61", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("DC7144", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("FC1F19", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert("1449E0", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("2C0E3D", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("3423BA", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("400E85", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("4C6641", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("54880E", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("843838", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("88329B", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("8CF5A3", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("AC5F3E", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("B479A7", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("BC8CCD", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("C09727", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("C0BDD1", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("C8BA94", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("D022BE", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("D02544", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("E8508B", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("EC1F72", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("EC9BF3", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("F025B7", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("F409D8", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("F8042E", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert("00E064", "SAMSUNG ELECTRONICS");
    map_macs.insert("0023C2", "SAMSUNG Electronics. Co. LTD");
    map_macs.insert("000DAE", "SAMSUNG HEAVY INDUSTRIES CO., LTD.");
    map_macs.insert("000918", "SAMSUNG TECHWIN CO.,LTD");
    map_macs.insert("842519", "Samsung Electronics");
    map_macs.insert("20DBAB", "Samsung Electronics Co., Ltd.");
    map_macs.insert("002538", "Samsung Electronics Co., Ltd., Memory Division");
    map_macs.insert("0000F0", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0007AB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001247", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001377", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001599", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0015B9", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001632", "Samsung Electronics Co.,Ltd");
    map_macs.insert("00166B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("00166C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0016DB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0017C9", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0017D5", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0018AF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001A8A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001B98", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001C43", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001D25", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001DF6", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001E7D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001EE1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001EE2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001FCC", "Samsung Electronics Co.,Ltd");
    map_macs.insert("001FCD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("00214C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0021D1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0021D2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("002339", "Samsung Electronics Co.,Ltd");
    map_macs.insert("00233A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("002399", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0023D6", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0023D7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("002454", "Samsung Electronics Co.,Ltd");
    map_macs.insert("002490", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0024E9", "Samsung Electronics Co.,Ltd");
    map_macs.insert("002566", "Samsung Electronics Co.,Ltd");
    map_macs.insert("00265D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("00265F", "Samsung Electronics Co.,Ltd");
    map_macs.insert("006F64", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0073E0", "Samsung Electronics Co.,Ltd");
    map_macs.insert("008701", "Samsung Electronics Co.,Ltd");
    map_macs.insert("00E3B2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("00F46F", "Samsung Electronics Co.,Ltd");
    map_macs.insert("04180F", "Samsung Electronics Co.,Ltd");
    map_macs.insert("041BBA", "Samsung Electronics Co.,Ltd");
    map_macs.insert("04FE31", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0808C2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0821EF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("08373D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("083D88", "Samsung Electronics Co.,Ltd");
    map_macs.insert("088C2C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("08D42B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("08ECA9", "Samsung Electronics Co.,Ltd");
    map_macs.insert("08EE8B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("08FC88", "Samsung Electronics Co.,Ltd");
    map_macs.insert("08FD0E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0C1420", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0C715D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0C8910", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0CB319", "Samsung Electronics Co.,Ltd");
    map_macs.insert("0CDFA4", "Samsung Electronics Co.,Ltd");
    map_macs.insert("101DC0", "Samsung Electronics Co.,Ltd");
    map_macs.insert("103047", "Samsung Electronics Co.,Ltd");
    map_macs.insert("103B59", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1077B1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("109266", "Samsung Electronics Co.,Ltd");
    map_macs.insert("10D38A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("10D542", "Samsung Electronics Co.,Ltd");
    map_macs.insert("141F78", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1432D1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("14568E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1489FD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("14A364", "Samsung Electronics Co.,Ltd");
    map_macs.insert("14B484", "Samsung Electronics Co.,Ltd");
    map_macs.insert("14BB6E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("14F42A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1816C9", "Samsung Electronics Co.,Ltd");
    map_macs.insert("181EB0", "Samsung Electronics Co.,Ltd");
    map_macs.insert("182195", "Samsung Electronics Co.,Ltd");
    map_macs.insert("18227E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("182666", "Samsung Electronics Co.,Ltd");
    map_macs.insert("183A2D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("183F47", "Samsung Electronics Co.,Ltd");
    map_macs.insert("184617", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1867B0", "Samsung Electronics Co.,Ltd");
    map_macs.insert("188331", "Samsung Electronics Co.,Ltd");
    map_macs.insert("18895B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("18E2C2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1C232C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1C3ADE", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1C5A3E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1C62B8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1C66AA", "Samsung Electronics Co.,Ltd");
    map_macs.insert("1CAF05", "Samsung Electronics Co.,Ltd");
    map_macs.insert("2013E0", "Samsung Electronics Co.,Ltd");
    map_macs.insert("202D07", "Samsung Electronics Co.,Ltd");
    map_macs.insert("205531", "Samsung Electronics Co.,Ltd");
    map_macs.insert("205EF7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("206E9C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("20D390", "Samsung Electronics Co.,Ltd");
    map_macs.insert("20D5BF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("244B03", "Samsung Electronics Co.,Ltd");
    map_macs.insert("244B81", "Samsung Electronics Co.,Ltd");
    map_macs.insert("24920E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("24C696", "Samsung Electronics Co.,Ltd");
    map_macs.insert("24DBED", "Samsung Electronics Co.,Ltd");
    map_macs.insert("24F5AA", "Samsung Electronics Co.,Ltd");
    map_macs.insert("2827BF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("28395E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("288335", "Samsung Electronics Co.,Ltd");
    map_macs.insert("28987B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("28BAB5", "Samsung Electronics Co.,Ltd");
    map_macs.insert("28CC01", "Samsung Electronics Co.,Ltd");
    map_macs.insert("2C4401", "Samsung Electronics Co.,Ltd");
    map_macs.insert("2CAE2B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("2CBABA", "Samsung Electronics Co.,Ltd");
    map_macs.insert("301966", "Samsung Electronics Co.,Ltd");
    map_macs.insert("3096FB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("30C7AE", "Samsung Electronics Co.,Ltd");
    map_macs.insert("30CBF8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("30CDA7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("30D587", "Samsung Electronics Co.,Ltd");
    map_macs.insert("30D6C9", "Samsung Electronics Co.,Ltd");
    map_macs.insert("34145F", "Samsung Electronics Co.,Ltd");
    map_macs.insert("343111", "Samsung Electronics Co.,Ltd");
    map_macs.insert("348A7B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("34AA8B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("34BE00", "Samsung Electronics Co.,Ltd");
    map_macs.insert("34C3AC", "Samsung Electronics Co.,Ltd");
    map_macs.insert("380195", "Samsung Electronics Co.,Ltd");
    map_macs.insert("380A94", "Samsung Electronics Co.,Ltd");
    map_macs.insert("380B40", "Samsung Electronics Co.,Ltd");
    map_macs.insert("3816D1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("382DD1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("382DE8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("389496", "Samsung Electronics Co.,Ltd");
    map_macs.insert("38D40B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("38ECE4", "Samsung Electronics Co.,Ltd");
    map_macs.insert("3C0518", "Samsung Electronics Co.,Ltd");
    map_macs.insert("3C5A37", "Samsung Electronics Co.,Ltd");
    map_macs.insert("3C6200", "Samsung Electronics Co.,Ltd");
    map_macs.insert("3C8BFE", "Samsung Electronics Co.,Ltd");
    map_macs.insert("3CA10D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("3CBBFD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("40163B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("40D3AE", "Samsung Electronics Co.,Ltd");
    map_macs.insert("444E1A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("446D6C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("44783E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("44F459", "Samsung Electronics Co.,Ltd");
    map_macs.insert("48137E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("4827EA", "Samsung Electronics Co.,Ltd");
    map_macs.insert("4844F7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("4849C7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("4C3C16", "Samsung Electronics Co.,Ltd");
    map_macs.insert("4CA56D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("4CBCA5", "Samsung Electronics Co.,Ltd");
    map_macs.insert("5001BB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("503275", "Samsung Electronics Co.,Ltd");
    map_macs.insert("503DA1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("5056BF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("508569", "Samsung Electronics Co.,Ltd");
    map_macs.insert("509EA7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("50A4C8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("50B7C3", "Samsung Electronics Co.,Ltd");
    map_macs.insert("50C8E5", "Samsung Electronics Co.,Ltd");
    map_macs.insert("50F0D3", "Samsung Electronics Co.,Ltd");
    map_macs.insert("50F520", "Samsung Electronics Co.,Ltd");
    map_macs.insert("50FC9F", "Samsung Electronics Co.,Ltd");
    map_macs.insert("5440AD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("5492BE", "Samsung Electronics Co.,Ltd");
    map_macs.insert("549B12", "Samsung Electronics Co.,Ltd");
    map_macs.insert("54F201", "Samsung Electronics Co.,Ltd");
    map_macs.insert("54FA3E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("58C38B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("5C2E59", "Samsung Electronics Co.,Ltd");
    map_macs.insert("5C3C27", "Samsung Electronics Co.,Ltd");
    map_macs.insert("5C497D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("5C9960", "Samsung Electronics Co.,Ltd");
    map_macs.insert("5CE8EB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("5CF6DC", "Samsung Electronics Co.,Ltd");
    map_macs.insert("606BBD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("6077E2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("608F5C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("60A10A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("60A4D0", "Samsung Electronics Co.,Ltd");
    map_macs.insert("60AF6D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("60C5AD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("60D0A9", "Samsung Electronics Co.,Ltd");
    map_macs.insert("646CB2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("647791", "Samsung Electronics Co.,Ltd");
    map_macs.insert("64B310", "Samsung Electronics Co.,Ltd");
    map_macs.insert("64B853", "Samsung Electronics Co.,Ltd");
    map_macs.insert("680571", "Samsung Electronics Co.,Ltd");
    map_macs.insert("682737", "Samsung Electronics Co.,Ltd");
    map_macs.insert("684898", "Samsung Electronics Co.,Ltd");
    map_macs.insert("68EBAE", "Samsung Electronics Co.,Ltd");
    map_macs.insert("6C2F2C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("6C8336", "Samsung Electronics Co.,Ltd");
    map_macs.insert("6CB7F4", "Samsung Electronics Co.,Ltd");
    map_macs.insert("6CF373", "Samsung Electronics Co.,Ltd");
    map_macs.insert("70288B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("70F927", "Samsung Electronics Co.,Ltd");
    map_macs.insert("74458A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("78009E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("781FDB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("7825AD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("7840E4", "Samsung Electronics Co.,Ltd");
    map_macs.insert("78471D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("78521A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("78595E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("789ED0", "Samsung Electronics Co.,Ltd");
    map_macs.insert("78A873", "Samsung Electronics Co.,Ltd");
    map_macs.insert("78ABBB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("78BDBC", "Samsung Electronics Co.,Ltd");
    map_macs.insert("78C3E9", "Samsung Electronics Co.,Ltd");
    map_macs.insert("78F7BE", "Samsung Electronics Co.,Ltd");
    map_macs.insert("7C0BC6", "Samsung Electronics Co.,Ltd");
    map_macs.insert("7C787E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("7C9122", "Samsung Electronics Co.,Ltd");
    map_macs.insert("7CF854", "Samsung Electronics Co.,Ltd");
    map_macs.insert("7CF90E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("8018A7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("804E81", "Samsung Electronics Co.,Ltd");
    map_macs.insert("805719", "Samsung Electronics Co.,Ltd");
    map_macs.insert("80656D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("84119E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("8425DB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("842E27", "Samsung Electronics Co.,Ltd");
    map_macs.insert("845181", "Samsung Electronics Co.,Ltd");
    map_macs.insert("8455A5", "Samsung Electronics Co.,Ltd");
    map_macs.insert("849866", "Samsung Electronics Co.,Ltd");
    map_macs.insert("84A466", "Samsung Electronics Co.,Ltd");
    map_macs.insert("84B541", "Samsung Electronics Co.,Ltd");
    map_macs.insert("888322", "Samsung Electronics Co.,Ltd");
    map_macs.insert("889B39", "Samsung Electronics Co.,Ltd");
    map_macs.insert("88ADD2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("8C1ABF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("8C71F8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("8C7712", "Samsung Electronics Co.,Ltd");
    map_macs.insert("8CBFA6", "Samsung Electronics Co.,Ltd");
    map_macs.insert("8CC8CD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9000DB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("900628", "Samsung Electronics Co.,Ltd");
    map_macs.insert("90F1AA", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9401C2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("94350A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("945103", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9463D1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9476B7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("948BC1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("94D771", "Samsung Electronics Co.,Ltd");
    map_macs.insert("981DFA", "Samsung Electronics Co.,Ltd");
    map_macs.insert("98398E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9852B1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("988389", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9C0298", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9C2A83", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9C3AAF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9C65B0", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9CD35B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("9CE6E7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A00798", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A01081", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A02195", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A06090", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A07591", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A0821F", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A0B4A5", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A0CBFD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A48431", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A49A58", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A4EBD3", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A80600", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A87C01", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A88195", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A89FBA", "Samsung Electronics Co.,Ltd");
    map_macs.insert("A8F274", "Samsung Electronics Co.,Ltd");
    map_macs.insert("AC3613", "Samsung Electronics Co.,Ltd");
    map_macs.insert("AC5A14", "Samsung Electronics Co.,Ltd");
    map_macs.insert("ACC33A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("ACEE9E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B047BF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B0C4E7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B0C559", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B0D09C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B0DF3A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B43A28", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B46293", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B47443", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B4EF39", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B857D8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B85A73", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B85E7B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B86CE8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B8BBAF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B8C68E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("B8D9CE", "Samsung Electronics Co.,Ltd");
    map_macs.insert("BC1485", "Samsung Electronics Co.,Ltd");
    map_macs.insert("BC20A4", "Samsung Electronics Co.,Ltd");
    map_macs.insert("BC4486", "Samsung Electronics Co.,Ltd");
    map_macs.insert("BC72B1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("BC765E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("BC79AD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("BC851F", "Samsung Electronics Co.,Ltd");
    map_macs.insert("BCB1F3", "Samsung Electronics Co.,Ltd");
    map_macs.insert("BCD11F", "Samsung Electronics Co.,Ltd");
    map_macs.insert("BCE63F", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C01173", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C06599", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C08997", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C0D3C0", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C44202", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C45006", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C4576E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C462EA", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C4731E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C488E5", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C4AE12", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C81479", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C819F7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C83870", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C87E75", "Samsung Electronics Co.,Ltd");
    map_macs.insert("C8A823", "Samsung Electronics Co.,Ltd");
    map_macs.insert("CC051B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("CC07AB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("CCB11A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("CCF9E8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("CCFE3C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D0176A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D059E4", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D0667B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D087E2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D0C1B1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D0DFC7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D0FCCC", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D47AE2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D487D8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D48890", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D4AE05", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D4E8B2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D831CF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D857EF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D85B2A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D890E8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D8C4E9", "Samsung Electronics Co.,Ltd");
    map_macs.insert("D8E0E1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("DC6672", "Samsung Electronics Co.,Ltd");
    map_macs.insert("DCCF96", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E09971", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E0CBEE", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E0DB10", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E4121D", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E432CB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E440E2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E458B8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E458E7", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E45D75", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E47CF9", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E47DBD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E492FB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E4B021", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E4E0C5", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E4F8EF", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E4FAED", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E8039A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E81132", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E83A12", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E84E84", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E89309", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E8B4C8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("E8E5D6", "Samsung Electronics Co.,Ltd");
    map_macs.insert("EC107B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("ECE09B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F008F1", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F05A09", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F05B7B", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F06BCA", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F0728C", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F0E77E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F0EE10", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F40E22", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F4428F", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F47B5E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F49F54", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F4D9FB", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F83F51", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F877B8", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F884F2", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F8D0BD", "Samsung Electronics Co.,Ltd");
    map_macs.insert("F8E61A", "Samsung Electronics Co.,Ltd");
    map_macs.insert("FC1910", "Samsung Electronics Co.,Ltd");
    map_macs.insert("FC4203", "Samsung Electronics Co.,Ltd");
    map_macs.insert("FC8F90", "Samsung Electronics Co.,Ltd");
    map_macs.insert("FCA13E", "Samsung Electronics Co.,Ltd");
    map_macs.insert("FCC734", "Samsung Electronics Co.,Ltd");
    map_macs.insert("FCF136", "Samsung Electronics Co.,Ltd");
    map_macs.insert("745F00", "Samsung Semiconductor Inc.");
    map_macs.insert("000DE5", "Samsung Thales");
    map_macs.insert("0022A6", "Sony Computer Entertainment America");
    map_macs.insert("00014A", "Sony Corporation");
    map_macs.insert("0013A9", "Sony Corporation");
    map_macs.insert("001A80", "Sony Corporation");
    map_macs.insert("001DBA", "Sony Corporation");
    map_macs.insert("0024BE", "Sony Corporation");
    map_macs.insert("045D4B", "Sony Corporation");
    map_macs.insert("080046", "Sony Corporation");
    map_macs.insert("104FA8", "Sony Corporation");
    map_macs.insert("30F9ED", "Sony Corporation");
    map_macs.insert("3C0771", "Sony Corporation");
    map_macs.insert("544249", "Sony Corporation");
    map_macs.insert("5453ED", "Sony Corporation");
    map_macs.insert("78843C", "Sony Corporation");
    map_macs.insert("AC9B0A", "Sony Corporation");
    map_macs.insert("D8D43C", "Sony Corporation");
    map_macs.insert("F0BF97", "Sony Corporation");
    map_macs.insert("FCF152", "Sony Corporation");
    map_macs.insert("00041F", "Sony Interactive Entertainment Inc.");
    map_macs.insert("001315", "Sony Interactive Entertainment Inc.");
    map_macs.insert("0015C1", "Sony Interactive Entertainment Inc.");
    map_macs.insert("0019C5", "Sony Interactive Entertainment Inc.");
    map_macs.insert("001FA7", "Sony Interactive Entertainment Inc.");
    map_macs.insert("00248D", "Sony Interactive Entertainment Inc.");
    map_macs.insert("00D9D1", "Sony Interactive Entertainment Inc.");
    map_macs.insert("0CFE45", "Sony Interactive Entertainment Inc.");
    map_macs.insert("280DFC", "Sony Interactive Entertainment Inc.");
    map_macs.insert("709E29", "Sony Interactive Entertainment Inc.");
    map_macs.insert("A8E3EE", "Sony Interactive Entertainment Inc.");
    map_macs.insert("BC60A7", "Sony Interactive Entertainment Inc.");
    map_macs.insert("F8461C", "Sony Interactive Entertainment Inc.");
    map_macs.insert("F8D0AC", "Sony Interactive Entertainment Inc.");
    map_macs.insert("FC0FE6", "Sony Interactive Entertainment Inc.");
    map_macs.insert("000AD9", "Sony Mobile Communications Inc");
    map_macs.insert("000E07", "Sony Mobile Communications Inc");
    map_macs.insert("000FDE", "Sony Mobile Communications Inc");
    map_macs.insert("0012EE", "Sony Mobile Communications Inc");
    map_macs.insert("001620", "Sony Mobile Communications Inc");
    map_macs.insert("0016B8", "Sony Mobile Communications Inc");
    map_macs.insert("001813", "Sony Mobile Communications Inc");
    map_macs.insert("001963", "Sony Mobile Communications Inc");
    map_macs.insert("001A75", "Sony Mobile Communications Inc");
    map_macs.insert("001B59", "Sony Mobile Communications Inc");
    map_macs.insert("001CA4", "Sony Mobile Communications Inc");
    map_macs.insert("001D28", "Sony Mobile Communications Inc");
    map_macs.insert("001E45", "Sony Mobile Communications Inc");
    map_macs.insert("001EDC", "Sony Mobile Communications Inc");
    map_macs.insert("001FE4", "Sony Mobile Communications Inc");
    map_macs.insert("00219E", "Sony Mobile Communications Inc");
    map_macs.insert("002298", "Sony Mobile Communications Inc");
    map_macs.insert("002345", "Sony Mobile Communications Inc");
    map_macs.insert("0023F1", "Sony Mobile Communications Inc");
    map_macs.insert("0024EF", "Sony Mobile Communications Inc");
    map_macs.insert("0025E7", "Sony Mobile Communications Inc");
    map_macs.insert("00EB2D", "Sony Mobile Communications Inc");
    map_macs.insert("18002D", "Sony Mobile Communications Inc");
    map_macs.insert("1C7B21", "Sony Mobile Communications Inc");
    map_macs.insert("205476", "Sony Mobile Communications Inc");
    map_macs.insert("2421AB", "Sony Mobile Communications Inc");
    map_macs.insert("283F69", "Sony Mobile Communications Inc");
    map_macs.insert("3017C8", "Sony Mobile Communications Inc");
    map_macs.insert("303926", "Sony Mobile Communications Inc");
    map_macs.insert("307512", "Sony Mobile Communications Inc");
    map_macs.insert("402BA1", "Sony Mobile Communications Inc");
    map_macs.insert("4040A7", "Sony Mobile Communications Inc");
    map_macs.insert("40B837", "Sony Mobile Communications Inc");
    map_macs.insert("44746C", "Sony Mobile Communications Inc");
    map_macs.insert("44D4E0", "Sony Mobile Communications Inc");
    map_macs.insert("4C21D0", "Sony Mobile Communications Inc");
    map_macs.insert("58170C", "Sony Mobile Communications Inc");
    map_macs.insert("584822", "Sony Mobile Communications Inc");
    map_macs.insert("5CB524", "Sony Mobile Communications Inc");
    map_macs.insert("68764F", "Sony Mobile Communications Inc");
    map_macs.insert("6C0E0D", "Sony Mobile Communications Inc");
    map_macs.insert("6C23B9", "Sony Mobile Communications Inc");
    map_macs.insert("8400D2", "Sony Mobile Communications Inc");
    map_macs.insert("848EDF", "Sony Mobile Communications Inc");
    map_macs.insert("84C7EA", "Sony Mobile Communications Inc");
    map_macs.insert("8C6422", "Sony Mobile Communications Inc");
    map_macs.insert("90C115", "Sony Mobile Communications Inc");
    map_macs.insert("94CE2C", "Sony Mobile Communications Inc");
    map_macs.insert("9C5CF9", "Sony Mobile Communications Inc");
    map_macs.insert("A0E453", "Sony Mobile Communications Inc");
    map_macs.insert("B4527D", "Sony Mobile Communications Inc");
    map_macs.insert("B4527E", "Sony Mobile Communications Inc");
    map_macs.insert("B8F934", "Sony Mobile Communications Inc");
    map_macs.insert("BC6E64", "Sony Mobile Communications Inc");
    map_macs.insert("C43ABE", "Sony Mobile Communications Inc");
    map_macs.insert("D05162", "Sony Mobile Communications Inc");
    map_macs.insert("E063E5", "Sony Mobile Communications Inc");
    map_macs.insert("04946B", "TECNO MOBILE LIMITED");
    map_macs.insert("088620", "TECNO MOBILE LIMITED");
    map_macs.insert("78FFCA", "TECNO MOBILE LIMITED");
    map_macs.insert("D47DFC", "TECNO MOBILE LIMITED");
    map_macs.insert("FC0012", "Toshiba Samsung Storage Technolgoy Korea Corporation");
    map_macs.insert("101212", "Vivo International Corporation Pty Ltd");
    map_macs.insert("6099D1", "Vuzix / Lenovo");
    map_macs.insert("00A0BF", "WIRELESS DATA GROUP MOTOROLA");
    map_macs.insert("286C07", "XIAOMI Electronics,CO.,LTD");
    map_macs.insert("34CE00", "XIAOMI Electronics,CO.,LTD");
    map_macs.insert("009EC8", "Xiaomi Communications Co Ltd");
    map_macs.insert("102AB3", "Xiaomi Communications Co Ltd");
    map_macs.insert("14F65A", "Xiaomi Communications Co Ltd");
    map_macs.insert("185936", "Xiaomi Communications Co Ltd");
    map_macs.insert("2082C0", "Xiaomi Communications Co Ltd");
    map_macs.insert("28E31F", "Xiaomi Communications Co Ltd");
    map_macs.insert("3480B3", "Xiaomi Communications Co Ltd");
    map_macs.insert("38A4ED", "Xiaomi Communications Co Ltd");
    map_macs.insert("584498", "Xiaomi Communications Co Ltd");
    map_macs.insert("640980", "Xiaomi Communications Co Ltd");
    map_macs.insert("64B473", "Xiaomi Communications Co Ltd");
    map_macs.insert("64CC2E", "Xiaomi Communications Co Ltd");
    map_macs.insert("68DFDD", "Xiaomi Communications Co Ltd");
    map_macs.insert("742344", "Xiaomi Communications Co Ltd");
    map_macs.insert("7451BA", "Xiaomi Communications Co Ltd");
    map_macs.insert("7802F8", "Xiaomi Communications Co Ltd");
    map_macs.insert("7C1DD9", "Xiaomi Communications Co Ltd");
    map_macs.insert("8CBEBE", "Xiaomi Communications Co Ltd");
    map_macs.insert("98FAE3", "Xiaomi Communications Co Ltd");
    map_macs.insert("9C99A0", "Xiaomi Communications Co Ltd");
    map_macs.insert("A086C6", "Xiaomi Communications Co Ltd");
    map_macs.insert("ACC1EE", "Xiaomi Communications Co Ltd");
    map_macs.insert("ACF7F3", "Xiaomi Communications Co Ltd");
    map_macs.insert("B0E235", "Xiaomi Communications Co Ltd");
    map_macs.insert("C40BCB", "Xiaomi Communications Co Ltd");
    map_macs.insert("C46AB7", "Xiaomi Communications Co Ltd");
    map_macs.insert("D4970B", "Xiaomi Communications Co Ltd");
    map_macs.insert("F0B429", "Xiaomi Communications Co Ltd");
    map_macs.insert("F48B32", "Xiaomi Communications Co Ltd");
    map_macs.insert("F8A45F", "Xiaomi Communications Co Ltd");
    map_macs.insert("FC64BA", "Xiaomi Communications Co Ltd");
    map_macs.insert("0823B2", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("10F681", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("18E29F", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("1CDA27", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("205D47", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("28FAA0", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("3CA348", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("3CB6B7", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("5419C8", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("6091F3", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("70D923", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("886AB1", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("9CA5C0", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("9CFBD5", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("BC2F3D", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("C46699", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("C4ABB2", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("DC1AC5", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("E0DDC0", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("E45AA2", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("ECDF3A", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("F01B6C", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("F42981", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("F470AB", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("FC1A11", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert("0015EB", "zte corporation");
    map_macs.insert("0019C6", "zte corporation");
    map_macs.insert("001E73", "zte corporation");
    map_macs.insert("002293", "zte corporation");
    map_macs.insert("002512", "zte corporation");
    map_macs.insert("0026ED", "zte corporation");
    map_macs.insert("004A77", "zte corporation");
    map_macs.insert("049573", "zte corporation");
    map_macs.insert("08181A", "zte corporation");
    map_macs.insert("083FBC", "zte corporation");
    map_macs.insert("0C1262", "zte corporation");
    map_macs.insert("143EBF", "zte corporation");
    map_macs.insert("146080", "zte corporation");
    map_macs.insert("1844E6", "zte corporation");
    map_macs.insert("18686A", "zte corporation");
    map_macs.insert("208986", "zte corporation");
    map_macs.insert("24C44A", "zte corporation");
    map_macs.insert("28FF3E", "zte corporation");
    map_macs.insert("2C26C5", "zte corporation");
    map_macs.insert("2C957F", "zte corporation");
    map_macs.insert("300C23", "zte corporation");
    map_macs.insert("30D386", "zte corporation");
    map_macs.insert("30F31D", "zte corporation");
    map_macs.insert("343759", "zte corporation");
    map_macs.insert("344B50", "zte corporation");
    map_macs.insert("344DEA", "zte corporation");
    map_macs.insert("346987", "zte corporation");
    map_macs.insert("34DE34", "zte corporation");
    map_macs.insert("34E0CF", "zte corporation");
    map_macs.insert("384608", "zte corporation");
    map_macs.insert("38D82F", "zte corporation");
    map_macs.insert("3CDA2A", "zte corporation");
    map_macs.insert("44F436", "zte corporation");
    map_macs.insert("48282F", "zte corporation");
    map_macs.insert("48A74E", "zte corporation");
    map_macs.insert("4C09B4", "zte corporation");
    map_macs.insert("4C16F1", "zte corporation");
    map_macs.insert("4CAC0A", "zte corporation");
    map_macs.insert("4CCBF5", "zte corporation");
    map_macs.insert("540955", "zte corporation");
    map_macs.insert("5422F8", "zte corporation");
    map_macs.insert("54BE53", "zte corporation");
    map_macs.insert("601466", "zte corporation");
    map_macs.insert("601888", "zte corporation");
    map_macs.insert("6073BC", "zte corporation");
    map_macs.insert("64136C", "zte corporation");
    map_macs.insert("681AB2", "zte corporation");
    map_macs.insert("688AF0", "zte corporation");
    map_macs.insert("689FF0", "zte corporation");
    map_macs.insert("6C8B2F", "zte corporation");
    map_macs.insert("6CA75F", "zte corporation");
    map_macs.insert("702E22", "zte corporation");
    map_macs.insert("709F2D", "zte corporation");
    map_macs.insert("744AA4", "zte corporation");
    map_macs.insert("749781", "zte corporation");
    map_macs.insert("74B57E", "zte corporation");
    map_macs.insert("78312B", "zte corporation");
    map_macs.insert("789682", "zte corporation");
    map_macs.insert("78C1A7", "zte corporation");
    map_macs.insert("78E8B6", "zte corporation");
    map_macs.insert("84742A", "zte corporation");
    map_macs.insert("88D274", "zte corporation");
    map_macs.insert("8C7967", "zte corporation");
    map_macs.insert("8CE081", "zte corporation");
    map_macs.insert("8CE117", "zte corporation");
    map_macs.insert("901D27", "zte corporation");
    map_macs.insert("90C7D8", "zte corporation");
    map_macs.insert("90D8F3", "zte corporation");
    map_macs.insert("94A7B7", "zte corporation");
    map_macs.insert("981333", "zte corporation");
    map_macs.insert("986CF5", "zte corporation");
    map_macs.insert("98F428", "zte corporation");
    map_macs.insert("98F537", "zte corporation");
    map_macs.insert("9CA9E4", "zte corporation");
    map_macs.insert("9CD24B", "zte corporation");
    map_macs.insert("A091C8", "zte corporation");
    map_macs.insert("A0EC80", "zte corporation");
    map_macs.insert("A8A668", "zte corporation");
    map_macs.insert("AC6462", "zte corporation");
    map_macs.insert("B075D5", "zte corporation");
    map_macs.insert("B49842", "zte corporation");
    map_macs.insert("B4B362", "zte corporation");
    map_macs.insert("B805AB", "zte corporation");
    map_macs.insert("C4A366", "zte corporation");
    map_macs.insert("C864C7", "zte corporation");
    map_macs.insert("C87B5B", "zte corporation");
    map_macs.insert("CC1AFA", "zte corporation");
    map_macs.insert("CC7B35", "zte corporation");
    map_macs.insert("D0154A", "zte corporation");
    map_macs.insert("D058A8", "zte corporation");
    map_macs.insert("D05BA8", "zte corporation");
    map_macs.insert("D0608C", "zte corporation");
    map_macs.insert("D071C4", "zte corporation");
    map_macs.insert("D437D7", "zte corporation");
    map_macs.insert("D476EA", "zte corporation");
    map_macs.insert("D4C1C8", "zte corporation");
    map_macs.insert("D855A3", "zte corporation");
    map_macs.insert("D87495", "zte corporation");
    map_macs.insert("DC028E", "zte corporation");
    map_macs.insert("E07C13", "zte corporation");
    map_macs.insert("E0C3F3", "zte corporation");
    map_macs.insert("E47723", "zte corporation");
    map_macs.insert("EC1D7F", "zte corporation");
    map_macs.insert("EC237B", "zte corporation");
    map_macs.insert("EC8A4C", "zte corporation");
    map_macs.insert("F084C9", "zte corporation");
    map_macs.insert("F41F88", "zte corporation");
    map_macs.insert("F46DE2", "zte corporation");
    map_macs.insert("F4B8A7", "zte corporation");
    map_macs.insert("F4E4AD", "zte corporation");
    map_macs.insert("F8A34F", "zte corporation");
    map_macs.insert("F8DFA8", "zte corporation");
    map_macs.insert("FC2D5E", "zte corporation");
    map_macs.insert("FCC897", "zte corporation");
    map_macs
}

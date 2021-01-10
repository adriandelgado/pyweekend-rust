use atoi::FromRadix10;
use chrono::NaiveDateTime;
use lazy_static::lazy_static;
use plotters::prelude::*;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
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

    let now0 = Instant::now();
    let comunes = fab_mas_comunes(dataset)?;
    println!("fab_mas_comunes: {} ms", now0.elapsed().as_millis());

    let now1 = Instant::now();
    generar_grafico(dataset)?;
    println!("generar_grafico: {} ms", now1.elapsed().as_millis());

    let now2 = Instant::now();
    let dic: TripleHashMap = total_bytes(dataset);
    println!("total_bytes: {} ms", now2.elapsed().as_millis());

    let mac_ap = "40A6E8:6C:5B:05";
    let timestamp = "1607173201";
    let now3 = Instant::now();
    let cli_unic = clientes_unicos(dataset, mac_ap, timestamp)?;
    println!("clientes_unicos: {} ms", now3.elapsed().as_millis());

    let mac_cliente = "4C3C16:46:65:62";
    let now4 = Instant::now();
    let cambios = cambio_edificio(dataset, aps_espol, mac_cliente)?;
    println!("cambio_edificio: {} ms", now4.elapsed().as_millis());

    for (fabricante, cantidad) in comunes {
        println!("{:<40} {:>6}", fabricante, cantidad);
    }

    println!("\n{:?}\n", dic.keys());

    println!("{:?}\n", cli_unic);

    println!("{:?}\n", cambios);

    Ok(())
}

fn csv_lines<P: AsRef<Path>>(csv: P) -> impl Iterator<Item = Result<Vec<u8>, io::Error>> {
    let file_handle = File::open(csv).expect("No se pudo abrir el CSV");
    let reader = BufReader::new(file_handle);
    reader.split('\n' as u8).skip(1)
}

fn fab_mas_comunes<P: AsRef<Path>>(dataset: P) -> io::Result<Vec<(String, u32)>> {
    let mut counter: FxHashMap<&str, u32> =
        HashMap::with_capacity_and_hasher(77, CustomHasher::default());
    let mut dispositivos_previos: FxHashSet<Vec<u8>> =
        HashSet::with_capacity_and_hasher(1000, CustomHasher::default());
    let dt = File::open(dataset)?;
    let mut reader_dt = BufReader::new(dt);
    let mut line = Vec::new();
    reader_dt.read_until('\n' as u8, &mut line)?;
    line.clear();

    loop {
        match reader_dt.read_until('\n' as u8, &mut line) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                if dispositivos_previos.insert(line[11..26].to_owned()) {
                    let mac_oui: &[u8; 6] = line[11..17].try_into().unwrap();
                    let fabricante = MAP_MACS
                        .get(mac_oui)
                        .expect(&format!("mac rara: {}", String::from_utf8_lossy(mac_oui)));
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

fn generar_grafico<P: AsRef<Path>>(dataset: P) -> io::Result<()> {
    let (fabricantes, cantidades): (Vec<String>, Vec<u32>) =
        fab_mas_comunes(dataset)?.into_iter().rev().unzip();
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
type TripleHashMap = FxHashMap<String, FxHashMap<String, FxHashMap<String, u32>>>;
fn total_bytes<P: AsRef<Path>>(dataset: P) -> TripleHashMap {
    let result: TripleHashMap = csv_lines(dataset)
        .par_bridge()
        .map(|line| {
            let line = line.unwrap();
            let timestamp = i64::from_radix_10(&line[..10]).0;
            let fecha = NaiveDateTime::from_timestamp(timestamp, 0)
                .date()
                .to_string();
            let mac_ap = String::from_utf8_lossy(&line[27..42]).into_owned();
            let bts = u32::from_radix_10(&line[43..49]).0;
            if line[50] == 'u' as u8 {
                (fecha, mac_ap, "recibidos".to_owned(), bts)
            } else {
                (fecha, mac_ap, "enviados".to_owned(), bts)
            }
        })
        .fold(
            || TripleHashMap::default(),
            |mut acc: TripleHashMap, (fecha, mac_ap, net, bts)| {
                *acc.entry(fecha)
                    .or_default()
                    .entry(mac_ap)
                    .or_default()
                    .entry(net)
                    .or_insert(0) += bts;
                acc
            },
        )
        .reduce(
            || TripleHashMap::default(),
            |mut this: TripleHashMap, other: TripleHashMap| {
                for (fecha, v1) in other.into_iter() {
                    for (mac_ap, v2) in v1.into_iter() {
                        for (net, bts) in v2.into_iter() {
                            *this
                                .entry(fecha.clone())
                                .or_default()
                                .entry(mac_ap.clone())
                                .or_default()
                                .entry(net)
                                .or_default() += bts;
                        }
                    }
                }
                this
            },
        );

    result
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
    let mut line = Vec::new();
    reader_dt.read_until('\n' as u8, &mut line)?;
    line.clear();
    loop {
        match reader_dt.read_until('\n' as u8, &mut line) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                if &line[27..42] == mac_ap.as_bytes()
                    && line[50] == 'u' as u8
                    && previas.insert(line[11..26].to_owned())
                {
                    let current = i64::from_radix_10(&line[..10]).0;
                    if 0 <= current - timestamp && current - timestamp <= 10800 {
                        let mac_cliente = String::from_utf8_lossy(&line[11..26]).into_owned();
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
        map_aps.insert(line[..15].to_owned(), line[16..19].to_owned());
    }
    let mut edificio_previo = Vec::new();
    let mut lista = Vec::new();
    let dt = File::open(dataset)?;
    let mut reader_dt = BufReader::new(dt);
    let mut line = Vec::new();
    reader_dt.read_until('\n' as u8, &mut line)?;
    line.clear();
    loop {
        match reader_dt.read_until('\n' as u8, &mut line) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                if &line[11..26] == mac_cliente.as_bytes() {
                    let edificio_actual = map_aps.get(&line[27..42]).unwrap();
                    if edificio_previo != *edificio_actual {
                        edificio_previo = edificio_actual.clone();
                        let timestamp = i64::from_radix_10(&line[..10]).0;
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

#[rustfmt::skip]
lazy_static! {
    static ref MAP_MACS: FxHashMap<&'static [u8; 6], &'static str> = {
    let mut map_macs = HashMap::with_capacity_and_hasher(1276, CustomHasher::default());
    map_macs.insert(b"00A081", "ALCATEL DATA NETWORKS");
    map_macs.insert(b"002060", "ALCATEL ITALIA S.p.A.");
    map_macs.insert(b"008039", "ALCATEL STC AUSTRALIA");
    map_macs.insert(b"002032", "ALCATEL TAISEL");
    map_macs.insert(b"00809F", "ALE International");
    map_macs.insert(b"00153F", "Alcatel Alenia Space Italia");
    map_macs.insert(b"000F62", "Alcatel Bell Space N.V.");
    map_macs.insert(b"00113F", "Alcatel DI");
    map_macs.insert(b"0CB5DE", "Alcatel Lucent");
    map_macs.insert(b"18422F", "Alcatel Lucent");
    map_macs.insert(b"4CA74B", "Alcatel Lucent");
    map_macs.insert(b"54055F", "Alcatel Lucent");
    map_macs.insert(b"68597F", "Alcatel Lucent");
    map_macs.insert(b"84A783", "Alcatel Lucent");
    map_macs.insert(b"885C47", "Alcatel Lucent");
    map_macs.insert(b"9067F3", "Alcatel Lucent");
    map_macs.insert(b"94AE61", "Alcatel Lucent");
    map_macs.insert(b"D4224E", "Alcatel Lucent");
    map_macs.insert(b"00089A", "Alcatel Microelectronics");
    map_macs.insert(b"000E86", "Alcatel North America");
    map_macs.insert(b"000502", "Apple, Inc.");
    map_macs.insert(b"00A040", "Apple, Inc.");
    map_macs.insert(b"080007", "Apple, Inc.");
    map_macs.insert(b"00040F", "Asus Network Technologies, Inc.");
    map_macs.insert(b"3CBD3E", "Beijing Xiaomi Electronics Co., Ltd.");
    map_macs.insert(b"08003E", "CODEX CORPORATION");
    map_macs.insert(b"1C48CE", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"1C77F6", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"2C5BB8", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"38295A", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"440444", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"4C1A3D", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"6C5C14", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"88D50C", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"8C0EE3", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"A09347", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"A43D78", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"A81B5A", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"B0AA36", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"B83765", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"BC3AEA", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"C09F05", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"C8F230", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"CC2D83", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"D4503F", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"DC6DCD", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"E44790", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"E8BBA8", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"EC01EE", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"ECF342", "GUANGDONG OPPO MOBILE TELECOMMUNICATIONS CORP.,LTD");
    map_macs.insert(b"00092D", "HTC Corporation");
    map_macs.insert(b"002376", "HTC Corporation");
    map_macs.insert(b"00EEBD", "HTC Corporation");
    map_macs.insert(b"04C23E", "HTC Corporation");
    map_macs.insert(b"188796", "HTC Corporation");
    map_macs.insert(b"1CB094", "HTC Corporation");
    map_macs.insert(b"2C8A72", "HTC Corporation");
    map_macs.insert(b"38E7D8", "HTC Corporation");
    map_macs.insert(b"404E36", "HTC Corporation");
    map_macs.insert(b"502E5C", "HTC Corporation");
    map_macs.insert(b"64A769", "HTC Corporation");
    map_macs.insert(b"7C6193", "HTC Corporation");
    map_macs.insert(b"800184", "HTC Corporation");
    map_macs.insert(b"807ABF", "HTC Corporation");
    map_macs.insert(b"847A88", "HTC Corporation");
    map_macs.insert(b"902155", "HTC Corporation");
    map_macs.insert(b"980D2E", "HTC Corporation");
    map_macs.insert(b"A0F450", "HTC Corporation");
    map_macs.insert(b"A826D9", "HTC Corporation");
    map_macs.insert(b"AC3743", "HTC Corporation");
    map_macs.insert(b"B4CEF6", "HTC Corporation");
    map_macs.insert(b"BCCFCC", "HTC Corporation");
    map_macs.insert(b"D40B1A", "HTC Corporation");
    map_macs.insert(b"D4206D", "HTC Corporation");
    map_macs.insert(b"D8B377", "HTC Corporation");
    map_macs.insert(b"E899C4", "HTC Corporation");
    map_macs.insert(b"F8DB7F", "HTC Corporation");
    map_macs.insert(b"001882", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"001E10", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"002568", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"00259E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"0034FE", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"00464B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"005A13", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"00664B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"009ACD", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"00E0FC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"00F81C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"04021F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"0425C5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"042758", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"043389", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"047503", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"049FCA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"04B0E7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"04BD70", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"04C06F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"04F938", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"04FE8D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"0819A6", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"086361", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"087A4C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"08C021", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"08E84F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"0C37DC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"0C45BA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"0C96BF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"0CD6BD", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"101B54", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"104780", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"105172", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"10B1F8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"10C61F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"143004", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"145F94", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"149D09", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"14A0F8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"14A51A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"14B968", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"14D11F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"18C58A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"18D276", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"18DED7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"1C1D67", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"1C6758", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"1C8E5C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"2008ED", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"200BC7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"202BC1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"20A680", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"20F17C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"20F3A3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"2400BA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"240995", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"241FA0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"244427", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"244C07", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"2469A5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"247F3C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"249EAB", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"24BCF8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"24DBAC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"24DF6A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"283152", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"283CE4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"285FDB", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"286ED4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"28B448", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"2C55D3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"2C9D1E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"2CAB00", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"307496", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"308730", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"30D17E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"30F335", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"3400A3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"341E6B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"346AC2", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"346BD3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"34A2A2", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"34B354", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"34CDBE", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"384C4F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"38BC01", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"38F889", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"3C4711", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"3C678C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"3CDFBD", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"3CF808", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"3CFA43", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"404D8E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"407D0F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"40CBA8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"4455B1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"446A2E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"446EE5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"4482E5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"44C346", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"480031", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"483C0C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"48435A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"486276", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"487B6B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"48AD08", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"48D539", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"48DB50", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"48FD8E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"4C1FCC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"4C5499", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"4C8BEF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"4CB16C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"4CF95D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"4CFB45", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"50016B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"5001D9", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"5004B8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"50680A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"509F27", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"50A72B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"5425EA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"5439DF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"54511B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"548998", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"54A51B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"581F28", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"582AF7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"58605F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"587F66", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"5C4CA9", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"5C7D5E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"5CA86A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"5CB395", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"5CB43E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"5CF96A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"600810", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"608334", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"60DE44", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"60E701", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"6416F0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"643E8C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"64A651", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"6889C1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"688F84", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"68A0F6", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"68A828", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"68CC6E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"7054F5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"70723C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"707990", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"707BE8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"708A09", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"70A8E3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"745AAA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"74882A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"749D8F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"74A063", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"74A528", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"781DBA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"786A89", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"78D752", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"78F557", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"78F5FD", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"7C11CB", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"7C1CF1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"7C6097", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"7C7D3D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"7CA23E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"7CB15D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"801382", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"8038BC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"80717A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"80B686", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"80D09B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"80D4A5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"80FB06", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"8421F1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"844765", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"845B12", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"849FB5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"84A8E4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"84A9C4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"84AD58", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"84BE52", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"84DBAC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"8828B3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"883FD3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"884477", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"8853D4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"886639", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"888603", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"88A2D7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"88CEFA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"88CF98", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"88E3AB", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"8C0D76", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"8C34FD", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"8CEBC6", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"900325", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"9017AC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"904E2B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"90671C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"94049C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"94772B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"94DBDA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"94FE22", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"98E7F5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"9C28EF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"9C37F4", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"9C52F8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"9C741A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"9C7DA3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"9CB2B2", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"9CC172", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"9CE374", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A0086F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A08CF8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A08D16", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A0A33B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A0F479", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A47174", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A49947", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A4BA76", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A4C64F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A4CAA0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A4DCBE", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A8C83A", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"A8CA7B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"AC4E91", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"AC6175", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"AC853D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"ACCF85", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"ACE215", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"ACE87B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"B05B67", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"B08900", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"B0E5ED", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"B41513", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"B43052", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"B808D7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"B8BC1B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"BC25E0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"BC3F8F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"BC620E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"BC7574", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"BC7670", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"BC9C31", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C07009", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C0BFC0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C40528", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C4072F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C4473F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C486E9", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C4F081", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C4FF1F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C80CC8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C81451", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C81FBE", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C85195", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C88D83", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C894BB", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"C8D15E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"CC53B5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"CC96A0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"CCA223", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"CCCC81", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D02DB3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D03E5C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D065CA", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D06F82", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D07AB5", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D0D04B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D0FF98", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D440F0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D4612E", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D46AA8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D46E5C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D494E8", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D4A148", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D4B110", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D4F9A1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D8490B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"D8C771", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"DC094C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"DCC64B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"DCD2FC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"DCD916", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"DCEE06", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E0191D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E0247F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E02861", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E03676", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E09796", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E0A3AC", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E468A3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E47E66", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E4A8B6", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E4C2D1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E8088B", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E84DD0", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E8BDD1", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"E8CD2D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"EC233D", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"EC388F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"EC4D47", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"ECCB30", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F02FA7", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F04347", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F09838", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F0C850", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F44C7F", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F4559C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F48E92", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F49FF3", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F4C714", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F4CB52", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F4DCF9", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F80113", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F823B2", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F83DFF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F84ABF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F87588", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F898B9", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F8BF09", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"F8E811", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"FC3F7C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"FC48EF", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"FCE33C", "HUAWEI TECHNOLOGIES CO.,LTD");
    map_macs.insert(b"0022A1", "Huawei Symantec Technologies Co.,Ltd.");
    map_macs.insert(b"B0989F", "LG CNS");
    map_macs.insert(b"F0182B", "LG Chem");
    map_macs.insert(b"3CE624", "LG Display");
    map_macs.insert(b"D84FB8", "LG ELECTRONICS");
    map_macs.insert(b"3CBDD8", "LG ELECTRONICS INC");
    map_macs.insert(b"3CCD93", "LG ELECTRONICS INC");
    map_macs.insert(b"9893CC", "LG ELECTRONICS INC");
    map_macs.insert(b"CC2D8C", "LG ELECTRONICS INC");
    map_macs.insert(b"E85B5B", "LG ELECTRONICS INC");
    map_macs.insert(b"00E091", "LG Electronics");
    map_macs.insert(b"14C913", "LG Electronics");
    map_macs.insert(b"388C50", "LG Electronics");
    map_macs.insert(b"6CD032", "LG Electronics");
    map_macs.insert(b"C808E9", "LG Electronics");
    map_macs.insert(b"001C62", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"001E75", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"001F6B", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"001FE3", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"0021FB", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"0022A9", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"002483", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"0025E5", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"0026E2", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"00AA70", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"0C4885", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"10683F", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"10F96F", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"2021A5", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"2C54CF", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"2C598A", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"30766F", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"344DF7", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"34FCEF", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"40B0FA", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"485929", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"505527", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"583F54", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"58A2B5", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"5C70A3", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"5CAF06", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"60E3AC", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"64899A", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"64BC0C", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"6CD68A", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"700514", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"74A722", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"78F882", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"805A04", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"88074B", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"88C9D0", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"8C3AE3", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"98D6F7", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"A039F7", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"A09169", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"A816B2", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"A8922C", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"A8B86E", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"AC0D1B", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"B81DAA", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"BCF5AC", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"C4438F", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"C49A02", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"CCFA00", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"D013FD", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"DC0B34", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"E892A4", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"F01C13", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"F80CF3", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"F8A9D0", "LG Electronics (Mobile Communications)");
    map_macs.insert(b"001256", "LG INFORMATION & COMM.");
    map_macs.insert(b"0019A1", "LG INFORMATION & COMM.");
    map_macs.insert(b"0050CE", "LG INTERNATIONAL CORP.");
    map_macs.insert(b"0005C9", "LG Innotek");
    map_macs.insert(b"001EB2", "LG Innotek");
    map_macs.insert(b"1C08C1", "LG Innotek");
    map_macs.insert(b"30A9DE", "LG Innotek");
    map_macs.insert(b"944444", "LG Innotek");
    map_macs.insert(b"A06FAA", "LG Innotek");
    map_macs.insert(b"C4366C", "LG Innotek");
    map_macs.insert(b"C80210", "LG Innotek");
    map_macs.insert(b"E8F2E2", "LG Innotek");
    map_macs.insert(b"A48CDB", "Lenovo");
    map_macs.insert(b"A03299", "Lenovo (Beijing) Co., Ltd.");
    map_macs.insert(b"207693", "Lenovo (Beijing) Limited.");
    map_macs.insert(b"74042B", "Lenovo Mobile Communication (Wuhan) Company Limited");
    map_macs.insert(b"E02CB2", "Lenovo Mobile Communication (Wuhan) Company Limited");
    map_macs.insert(b"1436C6", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"149FE8", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"503CC4", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"60D9A0", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"6C5F1C", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"70720D", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"80CF41", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"88708C", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"98FFD0", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"AC3870", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"C8DDC9", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"CC07E4", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"D87157", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"EC89F5", "Lenovo Mobile Communication Technology Ltd.");
    map_macs.insert(b"00E00C", "MOTOROLA");
    map_macs.insert(b"002075", "MOTOROLA COMMUNICATION ISRAEL");
    map_macs.insert(b"000A28", "Motorola");
    map_macs.insert(b"4888CA", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert(b"542758", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert(b"64DB43", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert(b"7C4685", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert(b"980CA5", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert(b"D0F88C", "Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.");
    map_macs.insert(b"002437", "Motorola - BSG");
    map_macs.insert(b"482CEA", "Motorola Inc Business Light Radios");
    map_macs.insert(b"000EC7", "Motorola Korea");
    map_macs.insert(b"141AA3", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"1430C6", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"24DA9B", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"34BB26", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"40786A", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"408805", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"4480EB", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"5C5188", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"60BEB5", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"68C44D", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"8058F8", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"806C1B", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"84100D", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"88797E", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"9068C3", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"9CD917", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"A470D6", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"B07994", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"CC61E5", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"CCC3EA", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"E0757D", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"E09861", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"E4907E", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"E89120", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"EC8892", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"F0D7AA", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"F4F1E1", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"F4F524", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"F8CFC5", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"F8E079", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"F8F1B6", "Motorola Mobility LLC, a Lenovo Company");
    map_macs.insert(b"4CCC34", "Motorola Solutions Inc.");
    map_macs.insert(b"0080D6", "NUVOTECH, INC.");
    map_macs.insert(b"00198F", "Nokia Bell N.V.");
    map_macs.insert(b"0002EE", "Nokia Danmark A/S");
    map_macs.insert(b"000EED", "Nokia Danmark A/S");
    map_macs.insert(b"000BE1", "Nokia NET Product Operations");
    map_macs.insert(b"000FBB", "Nokia Siemens Networks GmbH & Co. KG.");
    map_macs.insert(b"00061B", "Notebook Development Lab.  Lenovo Japan Ltd.");
    map_macs.insert(b"0022DE", "OPPO Digital, Inc.");
    map_macs.insert(b"C0EEFB", "OnePlus Tech (Shenzhen) Ltd");
    map_macs.insert(b"94652D", "OnePlus Technology (Shenzhen) Co., Ltd");
    map_macs.insert(b"000278", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"002119", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"002637", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"206432", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"38AA3C", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"50CCF8", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"5C0A5B", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"78D6F0", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"840B2D", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"90187C", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"980C82", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"A00BBA", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"B407F9", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"CC3A61", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"DC7144", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"FC1F19", "SAMSUNG ELECTRO MECHANICS CO., LTD.");
    map_macs.insert(b"1449E0", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"2C0E3D", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"3423BA", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"400E85", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"4C6641", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"54880E", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"843838", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"88329B", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"8CF5A3", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"AC5F3E", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"B479A7", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"BC8CCD", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"C09727", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"C0BDD1", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"C8BA94", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"D022BE", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"D02544", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"E8508B", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"EC1F72", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"EC9BF3", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"F025B7", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"F409D8", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"F8042E", "SAMSUNG ELECTRO-MECHANICS(THAILAND)");
    map_macs.insert(b"00E064", "SAMSUNG ELECTRONICS");
    map_macs.insert(b"0023C2", "SAMSUNG Electronics. Co. LTD");
    map_macs.insert(b"000DAE", "SAMSUNG HEAVY INDUSTRIES CO., LTD.");
    map_macs.insert(b"000918", "SAMSUNG TECHWIN CO.,LTD");
    map_macs.insert(b"842519", "Samsung Electronics");
    map_macs.insert(b"20DBAB", "Samsung Electronics Co., Ltd.");
    map_macs.insert(b"002538", "Samsung Electronics Co., Ltd., Memory Division");
    map_macs.insert(b"0000F0", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0007AB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001247", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001377", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001599", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0015B9", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001632", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"00166B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"00166C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0016DB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0017C9", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0017D5", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0018AF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001A8A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001B98", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001C43", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001D25", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001DF6", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001E7D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001EE1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001EE2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001FCC", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"001FCD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"00214C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0021D1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0021D2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"002339", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"00233A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"002399", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0023D6", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0023D7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"002454", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"002490", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0024E9", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"002566", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"00265D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"00265F", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"006F64", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0073E0", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"008701", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"00E3B2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"00F46F", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"04180F", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"041BBA", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"04FE31", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0808C2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0821EF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"08373D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"083D88", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"088C2C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"08D42B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"08ECA9", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"08EE8B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"08FC88", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"08FD0E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0C1420", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0C715D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0C8910", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0CB319", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"0CDFA4", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"101DC0", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"103047", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"103B59", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1077B1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"109266", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"10D38A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"10D542", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"141F78", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1432D1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"14568E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1489FD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"14A364", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"14B484", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"14BB6E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"14F42A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1816C9", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"181EB0", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"182195", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"18227E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"182666", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"183A2D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"183F47", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"184617", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1867B0", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"188331", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"18895B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"18E2C2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1C232C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1C3ADE", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1C5A3E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1C62B8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1C66AA", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"1CAF05", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"2013E0", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"202D07", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"205531", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"205EF7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"206E9C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"20D390", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"20D5BF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"244B03", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"244B81", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"24920E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"24C696", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"24DBED", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"24F5AA", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"2827BF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"28395E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"288335", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"28987B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"28BAB5", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"28CC01", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"2C4401", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"2CAE2B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"2CBABA", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"301966", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"3096FB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"30C7AE", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"30CBF8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"30CDA7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"30D587", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"30D6C9", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"34145F", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"343111", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"348A7B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"34AA8B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"34BE00", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"34C3AC", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"380195", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"380A94", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"380B40", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"3816D1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"382DD1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"382DE8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"389496", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"38D40B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"38ECE4", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"3C0518", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"3C5A37", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"3C6200", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"3C8BFE", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"3CA10D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"3CBBFD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"40163B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"40D3AE", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"444E1A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"446D6C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"44783E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"44F459", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"48137E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"4827EA", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"4844F7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"4849C7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"4C3C16", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"4CA56D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"4CBCA5", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"5001BB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"503275", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"503DA1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"5056BF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"508569", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"509EA7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"50A4C8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"50B7C3", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"50C8E5", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"50F0D3", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"50F520", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"50FC9F", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"5440AD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"5492BE", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"549B12", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"54F201", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"54FA3E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"58C38B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"5C2E59", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"5C3C27", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"5C497D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"5C9960", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"5CE8EB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"5CF6DC", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"606BBD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"6077E2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"608F5C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"60A10A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"60A4D0", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"60AF6D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"60C5AD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"60D0A9", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"646CB2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"647791", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"64B310", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"64B853", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"680571", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"682737", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"684898", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"68EBAE", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"6C2F2C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"6C8336", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"6CB7F4", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"6CF373", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"70288B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"70F927", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"74458A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"78009E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"781FDB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"7825AD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"7840E4", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"78471D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"78521A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"78595E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"789ED0", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"78A873", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"78ABBB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"78BDBC", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"78C3E9", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"78F7BE", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"7C0BC6", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"7C787E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"7C9122", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"7CF854", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"7CF90E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"8018A7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"804E81", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"805719", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"80656D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"84119E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"8425DB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"842E27", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"845181", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"8455A5", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"849866", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"84A466", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"84B541", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"888322", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"889B39", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"88ADD2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"8C1ABF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"8C71F8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"8C7712", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"8CBFA6", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"8CC8CD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9000DB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"900628", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"90F1AA", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9401C2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"94350A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"945103", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9463D1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9476B7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"948BC1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"94D771", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"981DFA", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"98398E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9852B1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"988389", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9C0298", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9C2A83", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9C3AAF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9C65B0", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9CD35B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"9CE6E7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A00798", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A01081", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A02195", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A06090", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A07591", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A0821F", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A0B4A5", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A0CBFD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A48431", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A49A58", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A4EBD3", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A80600", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A87C01", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A88195", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A89FBA", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"A8F274", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"AC3613", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"AC5A14", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"ACC33A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"ACEE9E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B047BF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B0C4E7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B0C559", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B0D09C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B0DF3A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B43A28", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B46293", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B47443", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B4EF39", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B857D8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B85A73", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B85E7B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B86CE8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B8BBAF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B8C68E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"B8D9CE", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"BC1485", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"BC20A4", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"BC4486", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"BC72B1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"BC765E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"BC79AD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"BC851F", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"BCB1F3", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"BCD11F", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"BCE63F", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C01173", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C06599", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C08997", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C0D3C0", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C44202", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C45006", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C4576E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C462EA", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C4731E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C488E5", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C4AE12", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C81479", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C819F7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C83870", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C87E75", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"C8A823", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"CC051B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"CC07AB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"CCB11A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"CCF9E8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"CCFE3C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D0176A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D059E4", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D0667B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D087E2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D0C1B1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D0DFC7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D0FCCC", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D47AE2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D487D8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D48890", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D4AE05", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D4E8B2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D831CF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D857EF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D85B2A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D890E8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D8C4E9", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"D8E0E1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"DC6672", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"DCCF96", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E09971", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E0CBEE", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E0DB10", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E4121D", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E432CB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E440E2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E458B8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E458E7", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E45D75", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E47CF9", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E47DBD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E492FB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E4B021", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E4E0C5", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E4F8EF", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E4FAED", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E8039A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E81132", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E83A12", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E84E84", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E89309", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E8B4C8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"E8E5D6", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"EC107B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"ECE09B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F008F1", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F05A09", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F05B7B", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F06BCA", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F0728C", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F0E77E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F0EE10", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F40E22", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F4428F", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F47B5E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F49F54", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F4D9FB", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F83F51", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F877B8", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F884F2", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F8D0BD", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"F8E61A", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"FC1910", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"FC4203", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"FC8F90", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"FCA13E", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"FCC734", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"FCF136", "Samsung Electronics Co.,Ltd");
    map_macs.insert(b"745F00", "Samsung Semiconductor Inc.");
    map_macs.insert(b"000DE5", "Samsung Thales");
    map_macs.insert(b"0022A6", "Sony Computer Entertainment America");
    map_macs.insert(b"00014A", "Sony Corporation");
    map_macs.insert(b"0013A9", "Sony Corporation");
    map_macs.insert(b"001A80", "Sony Corporation");
    map_macs.insert(b"001DBA", "Sony Corporation");
    map_macs.insert(b"0024BE", "Sony Corporation");
    map_macs.insert(b"045D4B", "Sony Corporation");
    map_macs.insert(b"080046", "Sony Corporation");
    map_macs.insert(b"104FA8", "Sony Corporation");
    map_macs.insert(b"30F9ED", "Sony Corporation");
    map_macs.insert(b"3C0771", "Sony Corporation");
    map_macs.insert(b"544249", "Sony Corporation");
    map_macs.insert(b"5453ED", "Sony Corporation");
    map_macs.insert(b"78843C", "Sony Corporation");
    map_macs.insert(b"AC9B0A", "Sony Corporation");
    map_macs.insert(b"D8D43C", "Sony Corporation");
    map_macs.insert(b"F0BF97", "Sony Corporation");
    map_macs.insert(b"FCF152", "Sony Corporation");
    map_macs.insert(b"00041F", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"001315", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"0015C1", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"0019C5", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"001FA7", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"00248D", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"00D9D1", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"0CFE45", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"280DFC", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"709E29", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"A8E3EE", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"BC60A7", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"F8461C", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"F8D0AC", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"FC0FE6", "Sony Interactive Entertainment Inc.");
    map_macs.insert(b"000AD9", "Sony Mobile Communications Inc");
    map_macs.insert(b"000E07", "Sony Mobile Communications Inc");
    map_macs.insert(b"000FDE", "Sony Mobile Communications Inc");
    map_macs.insert(b"0012EE", "Sony Mobile Communications Inc");
    map_macs.insert(b"001620", "Sony Mobile Communications Inc");
    map_macs.insert(b"0016B8", "Sony Mobile Communications Inc");
    map_macs.insert(b"001813", "Sony Mobile Communications Inc");
    map_macs.insert(b"001963", "Sony Mobile Communications Inc");
    map_macs.insert(b"001A75", "Sony Mobile Communications Inc");
    map_macs.insert(b"001B59", "Sony Mobile Communications Inc");
    map_macs.insert(b"001CA4", "Sony Mobile Communications Inc");
    map_macs.insert(b"001D28", "Sony Mobile Communications Inc");
    map_macs.insert(b"001E45", "Sony Mobile Communications Inc");
    map_macs.insert(b"001EDC", "Sony Mobile Communications Inc");
    map_macs.insert(b"001FE4", "Sony Mobile Communications Inc");
    map_macs.insert(b"00219E", "Sony Mobile Communications Inc");
    map_macs.insert(b"002298", "Sony Mobile Communications Inc");
    map_macs.insert(b"002345", "Sony Mobile Communications Inc");
    map_macs.insert(b"0023F1", "Sony Mobile Communications Inc");
    map_macs.insert(b"0024EF", "Sony Mobile Communications Inc");
    map_macs.insert(b"0025E7", "Sony Mobile Communications Inc");
    map_macs.insert(b"00EB2D", "Sony Mobile Communications Inc");
    map_macs.insert(b"18002D", "Sony Mobile Communications Inc");
    map_macs.insert(b"1C7B21", "Sony Mobile Communications Inc");
    map_macs.insert(b"205476", "Sony Mobile Communications Inc");
    map_macs.insert(b"2421AB", "Sony Mobile Communications Inc");
    map_macs.insert(b"283F69", "Sony Mobile Communications Inc");
    map_macs.insert(b"3017C8", "Sony Mobile Communications Inc");
    map_macs.insert(b"303926", "Sony Mobile Communications Inc");
    map_macs.insert(b"307512", "Sony Mobile Communications Inc");
    map_macs.insert(b"402BA1", "Sony Mobile Communications Inc");
    map_macs.insert(b"4040A7", "Sony Mobile Communications Inc");
    map_macs.insert(b"40B837", "Sony Mobile Communications Inc");
    map_macs.insert(b"44746C", "Sony Mobile Communications Inc");
    map_macs.insert(b"44D4E0", "Sony Mobile Communications Inc");
    map_macs.insert(b"4C21D0", "Sony Mobile Communications Inc");
    map_macs.insert(b"58170C", "Sony Mobile Communications Inc");
    map_macs.insert(b"584822", "Sony Mobile Communications Inc");
    map_macs.insert(b"5CB524", "Sony Mobile Communications Inc");
    map_macs.insert(b"68764F", "Sony Mobile Communications Inc");
    map_macs.insert(b"6C0E0D", "Sony Mobile Communications Inc");
    map_macs.insert(b"6C23B9", "Sony Mobile Communications Inc");
    map_macs.insert(b"8400D2", "Sony Mobile Communications Inc");
    map_macs.insert(b"848EDF", "Sony Mobile Communications Inc");
    map_macs.insert(b"84C7EA", "Sony Mobile Communications Inc");
    map_macs.insert(b"8C6422", "Sony Mobile Communications Inc");
    map_macs.insert(b"90C115", "Sony Mobile Communications Inc");
    map_macs.insert(b"94CE2C", "Sony Mobile Communications Inc");
    map_macs.insert(b"9C5CF9", "Sony Mobile Communications Inc");
    map_macs.insert(b"A0E453", "Sony Mobile Communications Inc");
    map_macs.insert(b"B4527D", "Sony Mobile Communications Inc");
    map_macs.insert(b"B4527E", "Sony Mobile Communications Inc");
    map_macs.insert(b"B8F934", "Sony Mobile Communications Inc");
    map_macs.insert(b"BC6E64", "Sony Mobile Communications Inc");
    map_macs.insert(b"C43ABE", "Sony Mobile Communications Inc");
    map_macs.insert(b"D05162", "Sony Mobile Communications Inc");
    map_macs.insert(b"E063E5", "Sony Mobile Communications Inc");
    map_macs.insert(b"04946B", "TECNO MOBILE LIMITED");
    map_macs.insert(b"088620", "TECNO MOBILE LIMITED");
    map_macs.insert(b"78FFCA", "TECNO MOBILE LIMITED");
    map_macs.insert(b"D47DFC", "TECNO MOBILE LIMITED");
    map_macs.insert(b"FC0012", "Toshiba Samsung Storage Technolgoy Korea Corporation");
    map_macs.insert(b"101212", "Vivo International Corporation Pty Ltd");
    map_macs.insert(b"6099D1", "Vuzix / Lenovo");
    map_macs.insert(b"00A0BF", "WIRELESS DATA GROUP MOTOROLA");
    map_macs.insert(b"286C07", "XIAOMI Electronics,CO.,LTD");
    map_macs.insert(b"34CE00", "XIAOMI Electronics,CO.,LTD");
    map_macs.insert(b"009EC8", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"102AB3", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"14F65A", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"185936", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"2082C0", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"28E31F", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"3480B3", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"38A4ED", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"584498", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"640980", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"64B473", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"64CC2E", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"68DFDD", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"742344", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"7451BA", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"7802F8", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"7C1DD9", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"8CBEBE", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"98FAE3", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"9C99A0", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"A086C6", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"ACC1EE", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"ACF7F3", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"B0E235", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"C40BCB", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"C46AB7", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"D4970B", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"F0B429", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"F48B32", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"F8A45F", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"FC64BA", "Xiaomi Communications Co Ltd");
    map_macs.insert(b"0823B2", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"10F681", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"18E29F", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"1CDA27", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"205D47", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"28FAA0", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"3CA348", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"3CB6B7", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"5419C8", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"6091F3", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"70D923", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"886AB1", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"9CA5C0", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"9CFBD5", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"BC2F3D", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"C46699", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"C4ABB2", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"DC1AC5", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"E0DDC0", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"E45AA2", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"ECDF3A", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"F01B6C", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"F42981", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"F470AB", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"FC1A11", "vivo Mobile Communication Co., Ltd.");
    map_macs.insert(b"0015EB", "zte corporation");
    map_macs.insert(b"0019C6", "zte corporation");
    map_macs.insert(b"001E73", "zte corporation");
    map_macs.insert(b"002293", "zte corporation");
    map_macs.insert(b"002512", "zte corporation");
    map_macs.insert(b"0026ED", "zte corporation");
    map_macs.insert(b"004A77", "zte corporation");
    map_macs.insert(b"049573", "zte corporation");
    map_macs.insert(b"08181A", "zte corporation");
    map_macs.insert(b"083FBC", "zte corporation");
    map_macs.insert(b"0C1262", "zte corporation");
    map_macs.insert(b"143EBF", "zte corporation");
    map_macs.insert(b"146080", "zte corporation");
    map_macs.insert(b"1844E6", "zte corporation");
    map_macs.insert(b"18686A", "zte corporation");
    map_macs.insert(b"208986", "zte corporation");
    map_macs.insert(b"24C44A", "zte corporation");
    map_macs.insert(b"28FF3E", "zte corporation");
    map_macs.insert(b"2C26C5", "zte corporation");
    map_macs.insert(b"2C957F", "zte corporation");
    map_macs.insert(b"300C23", "zte corporation");
    map_macs.insert(b"30D386", "zte corporation");
    map_macs.insert(b"30F31D", "zte corporation");
    map_macs.insert(b"343759", "zte corporation");
    map_macs.insert(b"344B50", "zte corporation");
    map_macs.insert(b"344DEA", "zte corporation");
    map_macs.insert(b"346987", "zte corporation");
    map_macs.insert(b"34DE34", "zte corporation");
    map_macs.insert(b"34E0CF", "zte corporation");
    map_macs.insert(b"384608", "zte corporation");
    map_macs.insert(b"38D82F", "zte corporation");
    map_macs.insert(b"3CDA2A", "zte corporation");
    map_macs.insert(b"44F436", "zte corporation");
    map_macs.insert(b"48282F", "zte corporation");
    map_macs.insert(b"48A74E", "zte corporation");
    map_macs.insert(b"4C09B4", "zte corporation");
    map_macs.insert(b"4C16F1", "zte corporation");
    map_macs.insert(b"4CAC0A", "zte corporation");
    map_macs.insert(b"4CCBF5", "zte corporation");
    map_macs.insert(b"540955", "zte corporation");
    map_macs.insert(b"5422F8", "zte corporation");
    map_macs.insert(b"54BE53", "zte corporation");
    map_macs.insert(b"601466", "zte corporation");
    map_macs.insert(b"601888", "zte corporation");
    map_macs.insert(b"6073BC", "zte corporation");
    map_macs.insert(b"64136C", "zte corporation");
    map_macs.insert(b"681AB2", "zte corporation");
    map_macs.insert(b"688AF0", "zte corporation");
    map_macs.insert(b"689FF0", "zte corporation");
    map_macs.insert(b"6C8B2F", "zte corporation");
    map_macs.insert(b"6CA75F", "zte corporation");
    map_macs.insert(b"702E22", "zte corporation");
    map_macs.insert(b"709F2D", "zte corporation");
    map_macs.insert(b"744AA4", "zte corporation");
    map_macs.insert(b"749781", "zte corporation");
    map_macs.insert(b"74B57E", "zte corporation");
    map_macs.insert(b"78312B", "zte corporation");
    map_macs.insert(b"789682", "zte corporation");
    map_macs.insert(b"78C1A7", "zte corporation");
    map_macs.insert(b"78E8B6", "zte corporation");
    map_macs.insert(b"84742A", "zte corporation");
    map_macs.insert(b"88D274", "zte corporation");
    map_macs.insert(b"8C7967", "zte corporation");
    map_macs.insert(b"8CE081", "zte corporation");
    map_macs.insert(b"8CE117", "zte corporation");
    map_macs.insert(b"901D27", "zte corporation");
    map_macs.insert(b"90C7D8", "zte corporation");
    map_macs.insert(b"90D8F3", "zte corporation");
    map_macs.insert(b"94A7B7", "zte corporation");
    map_macs.insert(b"981333", "zte corporation");
    map_macs.insert(b"986CF5", "zte corporation");
    map_macs.insert(b"98F428", "zte corporation");
    map_macs.insert(b"98F537", "zte corporation");
    map_macs.insert(b"9CA9E4", "zte corporation");
    map_macs.insert(b"9CD24B", "zte corporation");
    map_macs.insert(b"A091C8", "zte corporation");
    map_macs.insert(b"A0EC80", "zte corporation");
    map_macs.insert(b"A8A668", "zte corporation");
    map_macs.insert(b"AC6462", "zte corporation");
    map_macs.insert(b"B075D5", "zte corporation");
    map_macs.insert(b"B49842", "zte corporation");
    map_macs.insert(b"B4B362", "zte corporation");
    map_macs.insert(b"B805AB", "zte corporation");
    map_macs.insert(b"C4A366", "zte corporation");
    map_macs.insert(b"C864C7", "zte corporation");
    map_macs.insert(b"C87B5B", "zte corporation");
    map_macs.insert(b"CC1AFA", "zte corporation");
    map_macs.insert(b"CC7B35", "zte corporation");
    map_macs.insert(b"D0154A", "zte corporation");
    map_macs.insert(b"D058A8", "zte corporation");
    map_macs.insert(b"D05BA8", "zte corporation");
    map_macs.insert(b"D0608C", "zte corporation");
    map_macs.insert(b"D071C4", "zte corporation");
    map_macs.insert(b"D437D7", "zte corporation");
    map_macs.insert(b"D476EA", "zte corporation");
    map_macs.insert(b"D4C1C8", "zte corporation");
    map_macs.insert(b"D855A3", "zte corporation");
    map_macs.insert(b"D87495", "zte corporation");
    map_macs.insert(b"DC028E", "zte corporation");
    map_macs.insert(b"E07C13", "zte corporation");
    map_macs.insert(b"E0C3F3", "zte corporation");
    map_macs.insert(b"E47723", "zte corporation");
    map_macs.insert(b"EC1D7F", "zte corporation");
    map_macs.insert(b"EC237B", "zte corporation");
    map_macs.insert(b"EC8A4C", "zte corporation");
    map_macs.insert(b"F084C9", "zte corporation");
    map_macs.insert(b"F41F88", "zte corporation");
    map_macs.insert(b"F46DE2", "zte corporation");
    map_macs.insert(b"F4B8A7", "zte corporation");
    map_macs.insert(b"F4E4AD", "zte corporation");
    map_macs.insert(b"F8A34F", "zte corporation");
    map_macs.insert(b"F8DFA8", "zte corporation");
    map_macs.insert(b"FC2D5E", "zte corporation");
    map_macs.insert(b"FCC897", "zte corporation");
    map_macs
    };
}

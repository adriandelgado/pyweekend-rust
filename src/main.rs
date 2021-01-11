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
include!(concat!(env!("OUT_DIR"), "/map_oui.rs"));

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

use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::thread;

/// Open-addressing table with inline station keys (<= 100 bytes per the
/// spec), sized for the 10,000-unique-station ceiling. Capacity 16384
/// (>= 1.6x keys) keeps probe chains short.
const CAP: usize = 16384;
const NAME_MAX: usize = 100;

#[repr(C)]
struct Entry {
    len: u32,
    hash: u64,
    min: i32,
    max: i32,
    sum: i64,
    count: u64,
    name: [u8; NAME_MAX],
    used: bool,
}

impl Entry {
    fn new(name: &[u8], hash: u64, t: i32) -> Self {
        let mut nm = [0u8; NAME_MAX];
        nm[..name.len()].copy_from_slice(name);
        Entry {
            len: name.len() as u32,
            hash,
            min: t,
            max: t,
            sum: t as i64,
            count: 1,
            name: nm,
            used: true,
        }
    }
    fn matches(&self, name: &[u8], hash: u64) -> bool {
        self.hash == hash && self.len as usize == name.len() && &self.name[..name.len()] == name
    }
}

#[inline]
fn hash_name(name: &[u8]) -> u64 {
    // FxHash-style: multiply-rotate over 8-byte words
    let mut h: u64 = 0x51_7c_c1_b7_27_22_0a_95;
    let mut chunks = name.chunks_exact(8);
    for c in &mut chunks {
        let w = u64::from_le_bytes(c.try_into().unwrap());
        h = (h ^ w).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        let mut w = [0u8; 8];
        w[..rem.len()].copy_from_slice(rem);
        h = (h ^ u64::from_le_bytes(w)).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    h ^ (h >> 24)
}

struct Table {
    slots: Vec<Option<Entry>>,
    mask: usize,
}

impl Table {
    fn new() -> Self {
        Table { slots: (0..CAP).map(|_| None).collect(), mask: CAP - 1 }
    }

    fn upsert(&mut self, name: &[u8], hash: u64, t: i32) {
        let mut i = (hash as usize) & self.mask;
        loop {
            match &mut self.slots[i] {
                None => {
                    self.slots[i] = Some(Entry::new(name, hash, t));
                    return;
                }
                Some(e) if e.matches(name, hash) => {
                    if t < e.min {
                        e.min = t;
                    }
                    if t > e.max {
                        e.max = t;
                    }
                    e.sum += t as i64;
                    e.count += 1;
                    return;
                }
                _ => i = (i + 1) & self.mask,
            }
        }
    }

    fn merge(&mut self, other: Table) {
        for slot in other.slots {
            if let Some(e) = slot {
                let name_len = e.len as usize;
                let mut nm = [0u8; NAME_MAX];
                nm[..name_len].copy_from_slice(&e.name[..name_len]);
                let mut i = (e.hash as usize) & self.mask;
                loop {
                    match &mut self.slots[i] {
                        None => {
                            self.slots[i] = Some(Entry {
                                len: name_len as u32,
                                hash: e.hash,
                                min: e.min,
                                max: e.max,
                                sum: e.sum,
                                count: e.count,
                                name: nm,
                                used: true,
                            });
                            break;
                        }
                        Some(x) if x.matches(&e.name[..name_len], e.hash) => {
                            if e.min < x.min {
                                x.min = e.min;
                            }
                            if e.max > x.max {
                                x.max = e.max;
                            }
                            x.sum += e.sum;
                            x.count += e.count;
                            break;
                        }
                        _ => i = (i + 1) & self.mask,
                    }
                }
            }
        }
    }
}

/// Parse "12.3" / "-5.3" / "-0.8" into integer tenths, branch-light.
#[inline]
fn parse_tenths(b: &[u8]) -> i32 {
    debug_assert!(!b.is_empty());
    let (sign, mut i) = if b[0] == b'-' { (-1i32, 1usize) } else { (1i32, 0usize) };
    let mut int_part = 0i32;
    while i < b.len() && b[i] != b'.' {
        int_part = int_part * 10 + (b[i] - b'0') as i32;
        i += 1;
    }
    // exactly one fractional digit per the spec
    let frac = if i + 1 < b.len() { (b[i + 1] - b'0') as i32 } else { 0 };
    sign * (int_part * 10 + frac)
}

fn process_chunk(buf: &[u8], first_offset_in_file: bool) -> Table {
    let mut table = Table::new();
    let mut i = 0usize;
    // chunk starts mid-file: skip the first partial line (its other half
    // belongs to the previous chunk)
    if !first_offset_in_file {
        while i < buf.len() && buf[i] != b'\n' {
            i += 1;
        }
        i += 1; // past the newline
    }
    while i < buf.len() {
        // scan for ';'
        let semi = match buf[i..].iter().position(|&b| b == b';') {
            Some(p) => i + p,
            None => break,
        };
        let name = &buf[i..semi];
        // scan for '\n'
        let nl = match buf[semi..].iter().position(|&b| b == b'\n') {
            Some(p) => semi + p,
            None => buf.len(),
        };
        let temp = &buf[semi + 1..nl];
        if !name.is_empty() && !temp.is_empty() {
            if std::env::var_os("DBG").is_some() {
                eprintln!("DBG parse: name={:?} temp={:?} at i={semi}", String::from_utf8_lossy(name), String::from_utf8_lossy(temp));
            }
            let h = hash_name(name);
            let t = parse_tenths(temp);
            table.upsert(name, h, t);
        }
        i = nl + 1;
    }
    table
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let file_name = args.get(1).cloned().unwrap_or_else(|| "measurements.txt".into());

    let file = File::open(&file_name).expect("measurements file");
    let file_len = file.metadata().unwrap().len() as usize;
    let workers = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let chunk_target = (file_len / workers).max(1 << 20);

    let tables: Vec<Table> = thread::scope(|sc| {
        let mut joins = Vec::new();
        let mut start = 0usize;
        while start < file_len {
            let end = (start + chunk_target).min(file_len);
            let is_first = start == 0;
            let fname = file_name.clone();
            joins.push(sc.spawn(move || {
                let mut f = File::open(&fname).unwrap();
                use std::io::Seek;
                f.seek(io::SeekFrom::Start(start as u64)).unwrap();
                let to_read = end - start;
                let mut buf = vec![0u8; to_read];
                let mut filled = 0usize;
                while filled < to_read {
                    match f.read(&mut buf[filled..]) {
                        Ok(0) => break,
                        Ok(n) => filled += n,
                        Err(e) => panic!("read: {e}"),
                    }
                }
                process_chunk(&buf[..filled], is_first)
            }));
            start = end;
        }
        joins.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut base = Table::new();
    if std::env::var_os("DBG").is_some() {
        for (i, t) in tables.iter().enumerate() {
            let cnt = t.slots.iter().filter(|s| s.is_some()).count();
            eprintln!("DBG table {i}: {cnt} entries");
            for s in t.slots.iter().filter_map(|s| s.as_ref()) {
                eprintln!(
                    "  DBG {} min={} max={} sum={} count={}",
                    String::from_utf8_lossy(&s.name[..s.len as usize]),
                    s.min,
                    s.max,
                    s.sum,
                    s.count
                );
            }
        }
    }
    for t in tables {
        base.merge(t);
        if std::env::var_os("DBG").is_some() {
            let cnt = base.slots.iter().filter(|s| s.is_some()).count();
            eprintln!("DBG base after merge: {cnt}");
        }
    }

    // collect + sort by name bytes (matches UTF-8 String ordering)
    let mut rows: Vec<(&[u8], &Entry)> = base
        .slots
        .iter()
        .filter_map(|s| s.as_ref().map(|e| (&e.name[..e.len as usize], e)))
        .collect();
    rows.sort_unstable_by(|a, b| a.0.cmp(b.0));

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    write!(out, "{{").unwrap();
    let mut sep = "";
    for (name, e) in &rows {
        // mathematical rounding, integer-exact (mirrors the baseline)
        let mut avg = (e.sum * 10 / e.count as i64) as i32;
        if avg.abs() % 10 >= 5 {
            avg += 10;
        }
        avg /= 10;
        write!(
            out,
            "{sep}{}={}{}.{}/{}{}.{}/{}{}.{}",
            String::from_utf8_lossy(name),
            if e.min < 0 { "-" } else { "" },
            e.min.abs() / 10,
            e.min.abs() % 10,
            if avg < 0 { "-" } else { "" },
            avg.abs() / 10,
            avg.abs() % 10,
            if e.max < 0 { "-" } else { "" },
            e.max.abs() / 10,
            e.max.abs() % 10
        )
        .unwrap();
        sep = ", ";
    }
    writeln!(out, "}}").unwrap();
    out.flush().unwrap();
}

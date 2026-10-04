use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub struct LatencyRecord {
    pub thread_id: u64,
    pub request_id: u64,
    // when request was supposed to be send
    pub scheduled_ns: u64,
    // when request was actually sent
    pub send_ns: u64,
    // when response was received 
    pub recv_ns: u64,
    // recv_ns - scheduled_ns
    pub latency_ns: u64,
    pub server_time_ns: u64,
    pub payload_len: u64,
}

pub const CSV_HEADER: &str =
    "thread_id,request_id,scheduled_ns,send_ns,recv_ns,latency_ns,server_time_ns,payload_len";

pub fn ns_since(start: Instant) -> u64 {
    start.elapsed().as_nanos() as u64
}

pub fn write_csv(records: &[LatencyRecord], path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut w = BufWriter::new(File::create(path)?);
    writeln!(w, "{CSV_HEADER}")?;
    for r in records {
        writeln!(
            w,
            "{},{},{},{},{},{},{},{}",
            r.thread_id,
            r.request_id,
            r.scheduled_ns,
            r.send_ns,
            r.recv_ns,
            r.latency_ns,
            r.server_time_ns,
            r.payload_len,
        )?;
    }
    w.flush()
}

pub fn print_summary(records: &[LatencyRecord]) {
    if records.is_empty() {
        println!("no requests completed");
        return;
    }

    let first_send = records.iter().map(|r| r.send_ns).min().unwrap();
    let last_recv = records.iter().map(|r| r.recv_ns).max().unwrap();
    let window_secs = (last_recv - first_send) as f64 / 1e9;

    let mut lat: Vec<u64> = records.iter().map(|r| r.latency_ns).collect();
    lat.sort_unstable();
    let pct = |p: f64| lat[((lat.len() - 1) as f64 * p).round() as usize] as f64 / 1e3;

    println!("requests_completed={}", records.len());
    println!("window_secs={window_secs:.3}");
    println!("achieved_rps={:.1}", records.len() as f64 / window_secs);
    println!("p50_us={:.2}", pct(0.50));
    println!("p95_us={:.2}", pct(0.95));
    println!("p99_us={:.2}", pct(0.99));
}
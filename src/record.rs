//! Per-request latency records, and writing them to a CSV in --outpath.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub struct LatencyRecord {

    pub thread_id: u64,
    pub request_id: u64,
    pub weight: u64,
    pub scheduled_ns: u64,
    pub send_ns: u64,
    pub response: Option<ResponseInfo>,
}

#[derive(Debug, Clone, Copy)]
pub struct ResponseInfo {
    pub recv_ns: u64,
    pub server_time_ns: u64,
    pub server_queue_ns: u64,
    pub payload_len: u64,
}

impl LatencyRecord {
    pub fn latency_ns(&self) -> Option<u64> {
        self.response.map(|r| r.recv_ns.saturating_sub(self.scheduled_ns))
    }
}

pub const CSV_HEADER: &str = "thread_id,request_id,weight,scheduled_ns,send_ns,recv_ns,\
latency_ns,server_time_ns,server_queue_ns,payload_len";

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
        write!(
            w,
            "{},{},{},{},{},",
            r.thread_id, r.request_id, r.weight, r.scheduled_ns, r.send_ns
        )?;
        match (r.response, r.latency_ns()) {
            (Some(resp), Some(lat)) => writeln!(
                w,
                "{},{},{},{},{}",
                resp.recv_ns, lat, resp.server_time_ns, resp.server_queue_ns, resp.payload_len
            )?,
            _ => writeln!(w, ",,,,")?,
        }
    }
    w.flush()
}

pub fn print_summary(records: &[LatencyRecord]) {
    if records.is_empty() {
        println!("no requests sent");
        return;
    }
    let weighted = |it: &mut dyn Iterator<Item = &LatencyRecord>| -> u64 { it.map(|r| r.weight).sum() };

    let sent = weighted(&mut records.iter());
    let completed = weighted(&mut records.iter().filter(|r| r.response.is_some()));

    let first_sched = records.iter().map(|r| r.scheduled_ns).min().unwrap();
    let last_sched = records.iter().map(|r| r.scheduled_ns).max().unwrap();
    let first_send = records.iter().map(|r| r.send_ns).min().unwrap();
    let last_send = records.iter().map(|r| r.send_ns).max().unwrap();
    let last_recv = records.iter().filter_map(|r| r.response).map(|r| r.recv_ns).max();

    let rate = |n: u64, from: u64, to: u64| {
        let secs = to.saturating_sub(from) as f64 / 1e9;
        if secs > 0.0 { n as f64 / secs } else { f64::NAN }
    };

    println!("requests_sent={sent}");
    println!("requests_completed={completed}");
    println!("requests_unanswered={}", sent - completed);
    println!("attempted_rps={:.1}", rate(sent, first_sched, last_sched));
    println!("offered_rps={:.1}", rate(sent, first_send, last_send));
    if let Some(last_recv) = last_recv {
        println!("achieved_rps={:.1}", rate(completed, first_send, last_recv));
    }

    let mut lat: Vec<u64> = records.iter().filter_map(|r| r.latency_ns()).collect();
    if lat.is_empty() {
        return;
    }
    lat.sort_unstable();
    let pct = |p: f64| lat[((lat.len() - 1) as f64 * p).round() as usize] as f64 / 1e3;
    println!("p50_us={:.2}", pct(0.50));
    println!("p95_us={:.2}", pct(0.95));
    println!("p99_us={:.2}", pct(0.99));
}

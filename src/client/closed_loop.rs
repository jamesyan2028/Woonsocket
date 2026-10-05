
use std::io;
use std::net::{Ipv4Addr, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use woonsocket_work::args::WoonsocketClientOpt;
use woonsocket_work::Work;

use crate::framing;
use crate::protocol::{Request, Response};
use crate::record::{self, ns_since, LatencyRecord};

pub fn run(opts: &WoonsocketClientOpt, num_threads: u64) -> io::Result<()> {
    let start = Instant::now();
    let deadline = start + Duration::from_secs(opts.runtime_secs);

    let mut handles = Vec::with_capacity(num_threads as usize);
    for thread_id in 0..num_threads {
        let (ip, port, work) = (opts.ip, opts.port, opts.work);
        handles.push(thread::spawn(move || {
            client_thread(thread_id, ip, port, work, start, deadline)
        }));
    }

    let mut records = Vec::new();
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(mut recs) => records.append(&mut recs),
            Err(_) => eprintln!("thread {i} panicked"),
        }
    }
    records.sort_by_key(|r| r.send_ns);

    let path = opts.outpath.join(format!("closed-loop-{num_threads}.csv"));
    record::write_csv(&records, &path)?;
    println!("mode=closed-loop num_threads={num_threads} work={}", opts.work);
    record::print_summary(&records);
    Ok(())
}

fn client_thread(
    thread_id: u64,
    ip: Ipv4Addr,
    port: u16,
    work: Work,
    start: Instant,
    deadline: Instant,
) -> Vec<LatencyRecord> {
    let mut records = Vec::with_capacity(1 << 16);

    let mut stream = match connect(ip, port, deadline) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("thread {thread_id}: connect failed: {e}");
            return records;
        }
    };

    let mut request_id = 0u64;
    while Instant::now() < deadline {
        let send_ns = ns_since(start);

        let req = Request { id: request_id, work };
        if let Err(e) = framing::send(&mut stream, &req) {
            eprintln!("thread {thread_id}: send failed: {e}");
            break;
        }

        let resp: Response = match framing::receive(&mut stream) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("thread {thread_id}: receive failed: {e}");
                break;
            }
        };
        let recv_ns = ns_since(start);

        if resp.id != request_id {
            eprintln!(
                "thread {thread_id}: response id {} != request id {request_id}",
                resp.id
            );
            break;
        }

        records.push(LatencyRecord {
            thread_id,
            request_id,
            scheduled_ns: send_ns,
            send_ns,
            recv_ns,
            latency_ns: recv_ns - send_ns,
            server_time_ns: resp.server_time_ns,
            payload_len: resp.payload.len() as u64,
        });
        request_id += 1;
    }

    records
}

fn connect(ip: Ipv4Addr, port: u16, deadline: Instant) -> io::Result<TcpStream> {
    loop {
        match TcpStream::connect((ip, port)) {
            Ok(stream) => {
                stream.set_nodelay(true)?;
                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                return Ok(stream);
            }
            Err(_) if Instant::now() + Duration::from_millis(100) < deadline => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
}
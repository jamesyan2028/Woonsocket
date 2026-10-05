use std::io::{self, BufReader, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use woonsocket_work::args::{OpenLoopKind, WoonsocketClientOpt};
use woonsocket_work::Work;

use crate::arrivals::Arrivals;
use crate::client::closed_loop::connect;
use crate::framing;
use crate::protocol::{Request, Response};
use crate::record::{self, ns_since, LatencyRecord, ResponseInfo};

const NUM_CONNECTIONS: u64 = 4;
const MAX_DETAILED_RECORDS: f64 = 1_000_000.0;
const MAX_BATCH: usize = 256;
const DRAIN_GRACE: Duration = Duration::from_millis(500);


// data structure to track number of globally sent/received requests
#[derive(Default)]
struct ConnCounters {
    sent: AtomicU64,
    received: AtomicU64,
}

struct SentSample {
    seq: u64, 
    scheduled_ns: u64,
    send_ns: u64,
}

pub fn run(opts: &WoonsocketClientOpt, interval_us: u64, kind: OpenLoopKind) -> io::Result<()> {
    if interval_us == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "--interval-us must be > 0"));
    }
    let kind_name = match kind {
        OpenLoopKind::Constant => "constant",
        OpenLoopKind::Poisson => "poisson",
    };
    let k = NUM_CONNECTIONS as usize;

    let interval_ns = interval_us as f64 * 1000.0;
    let expected_total = opts.runtime_secs as f64 * 1e9 / interval_ns;
    let sample_every = (expected_total / MAX_DETAILED_RECORDS).ceil().max(1.0) as u64;

    let connect_deadline = Instant::now() + Duration::from_secs(opts.runtime_secs);
    let mut streams = Vec::with_capacity(k);
    for _ in 0..k {
        streams.push(connect(opts.ip, opts.port, connect_deadline)?);
    }

    let start = Instant::now();
    let deadline_ns = opts.runtime_secs * 1_000_000_000;
    let counters: Vec<Arc<ConnCounters>> = (0..k).map(|_| Arc::default()).collect();

    let mut stoppers = Vec::with_capacity(k);
    let mut receivers = Vec::with_capacity(k);
    for (c, stream) in streams.iter().enumerate() {
        stoppers.push(stream.try_clone()?);
        let reader = BufReader::with_capacity(1 << 16, stream.try_clone()?);
        let counters = Arc::clone(&counters[c]);
        receivers.push(thread::spawn(move || {
            receiver(c as u64, reader, start, sample_every, counters)
        }));
    }

    let arrivals = match kind {
        OpenLoopKind::Constant => Arrivals::constant(interval_ns, 0.0),
        OpenLoopKind::Poisson => Arrivals::poisson(interval_ns),
    };
    let sender = {
        let counters = counters.clone();
        let work = opts.work;
        thread::spawn(move || {
            sender(streams, arrivals, work, start, deadline_ns, sample_every, &counters)
        })
    };

    let hard_stop = start + Duration::from_secs(opts.runtime_secs) + DRAIN_GRACE;
    while !sender.is_finished() && Instant::now() < hard_stop {
        thread::sleep(Duration::from_millis(1));
    }

    let drain_until = Instant::now() + DRAIN_GRACE;
    while Instant::now() < drain_until {
        let all_done = counters.iter().zip(&receivers).all(|(c, r)| {
            r.is_finished() || c.received.load(Ordering::Acquire) >= c.sent.load(Ordering::Acquire)
        });
        if all_done {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    for s in &stoppers {
        let _ = s.shutdown(Shutdown::Both);
    }
    let sent_samples = sender.join().unwrap_or_else(|_| {
        eprintln!("sender panicked");
        (0..k).map(|_| Vec::new()).collect()
    });
    let mut responses = Vec::with_capacity(k);
    for (c, r) in receivers.into_iter().enumerate() {
        responses.push(r.join().unwrap_or_else(|_| {
            eprintln!("receiver {c} panicked");
            Vec::new()
        }));
    }

    let mut records = Vec::new();
    for (c, (sent, resps)) in sent_samples.into_iter().zip(responses).enumerate() {
        for (i, s) in sent.into_iter().enumerate() {
            let response = match resps.get(i) {
                Some(&(seq, info)) if seq == s.seq => Some(info),
                Some(&(seq, _)) => {
                    eprintln!("conn {c}: sample mismatch (sent seq {}, response seq {seq})", s.seq);
                    None
                }
                None => None,
            };
            records.push(LatencyRecord {
                thread_id: c as u64,
                request_id: s.seq,
                weight: sample_every,
                scheduled_ns: s.scheduled_ns,
                send_ns: s.send_ns,
                response,
            });
        }
    }
    records.sort_by_key(|r| r.scheduled_ns);

    let path = opts.outpath.join(format!("open-loop-{kind_name}-{interval_us}.csv"));
    record::write_csv(&records, &path)?;
    println!(
        "mode=open-loop kind={kind_name} interval_us={interval_us} work={} connections={k} sample_every={sample_every} cpus={}",
        opts.work,
        thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
    );
    println!("target_rps={:.1}", 1e9 / interval_ns);
    record::print_summary(&records);
    Ok(())
}

fn sender(
    mut streams: Vec<TcpStream>,
    mut arrivals: Arrivals,
    work: Work,
    start: Instant,
    deadline_ns: u64,
    sample_every: u64,
    counters: &[Arc<ConnCounters>],
) -> Vec<Vec<SentSample>> {
    let k = streams.len();
    let mut samples: Vec<Vec<SentSample>> = (0..k).map(|_| Vec::new()).collect();
    let mut bufs: Vec<Vec<u8>> = (0..k).map(|_| Vec::with_capacity(MAX_BATCH * 32)).collect();
    let mut pending: Vec<Vec<(u64, u64)>> = (0..k).map(|_| Vec::new()).collect();
    let mut next_seq = vec![0u64; k];

    let mut global = 0u64; 
    let mut next = arrivals.next().unwrap();

    'run: while next < deadline_ns {
        let now = wait_until(start, next);

        let mut batch = 0;
        while next <= now && next < deadline_ns && batch < MAX_BATCH {
            let c = (global % k as u64) as usize;
            let seq = next_seq[c];
            next_seq[c] += 1;
            if let Err(e) = framing::encode(&mut bufs[c], &Request { id: seq, work }) {
                eprintln!("encode failed: {e}");
                break 'run;
            }

            if seq % sample_every == 0 {
                pending[c].push((seq, next));
            }
            global += 1;
            batch += 1;
            next = arrivals.next().unwrap();
        }

        for c in 0..k {
            if bufs[c].is_empty() {
                continue;
            }

            if let Err(e) = streams[c].write_all(&bufs[c]) {
                eprintln!("conn {c}: send failed: {e}");
                break 'run;
            }
            let send_ns = ns_since(start);
            bufs[c].clear();
            for (seq, scheduled_ns) in pending[c].drain(..) {
                samples[c].push(SentSample { seq, scheduled_ns, send_ns });
            }
            counters[c].sent.store(next_seq[c], Ordering::Release);
        }
    }

    samples
}

fn wait_until(start: Instant, target_ns: u64) -> u64 {
    loop {
        let now = ns_since(start);
        if now >= target_ns {
            return now;
        }
        let remaining = target_ns - now;
        if remaining > 200_000 {
            thread::sleep(Duration::from_nanos(remaining - 100_000));
        } else {
            std::hint::spin_loop();
        }
    }
}

fn receiver(
    conn: u64,
    mut reader: BufReader<TcpStream>,
    start: Instant,
    sample_every: u64,
    counters: Arc<ConnCounters>,
) -> Vec<(u64, ResponseInfo)> {
    let mut out = Vec::new();
    let mut expected = 0u64;
    loop {
        let resp: Response = match framing::receive(&mut reader) {
            Ok(r) => r,
            Err(_) => break, 
        };
        let recv_ns = ns_since(start);

        if resp.id != expected {
            eprintln!("conn {conn}: response id {} != expected {expected}", resp.id);
            break;
        }

        
        if expected % sample_every == 0 {
            out.push((
                expected,
                ResponseInfo {
                    recv_ns,
                    server_time_ns: resp.server_time_ns,
                    server_queue_ns: resp.server_queue_ns,
                    payload_len: resp.payload.len() as u64,
                },
            ));
        }
        expected += 1;
        counters.received.store(expected, Ordering::Release);
    }
    out
}

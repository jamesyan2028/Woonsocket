use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use woonsocket::framing;
use woonsocket::protocol::{Request, Response};
use woonsocket_work::args::WoonsocketServerOpt;

const MAX_RESPONSES_PER_FLUSH: usize = 64;

fn handle_connection(stream: TcpStream, served: Arc<AtomicU64>) {
    if let Err(e) = stream.set_nodelay(true) {
        eprintln!("set_nodelay failed: {e}");
    }
    let read_half = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("try_clone failed: {e}");
            return;
        }
    };

    let (tx, rx) = mpsc::channel::<(Request, Instant)>();
    thread::spawn(move || reader(read_half, tx));
    worker(stream, rx, served);
}
fn reader(stream: TcpStream, tx: Sender<(Request, Instant)>) {
    let mut r = BufReader::with_capacity(1 << 16, stream);
    loop {
        let req: Request = match framing::receive(&mut r) {
            Ok(req) => req,
            Err(_) => return, 
        };
        if tx.send((req, Instant::now())).is_err() {
            return; 
        }
    }
}

fn worker(stream: TcpStream, rx: Receiver<(Request, Instant)>, served: Arc<AtomicU64>) {
    let mut w = BufWriter::with_capacity(1 << 16, stream);

    while let Ok(first) = rx.recv() {
        let mut item = first;
        let mut since_flush = 0;
        loop {
            if handle_request(&mut w, item).is_err() {
                return; 
            }
            served.fetch_add(1, Ordering::Relaxed);
            since_flush += 1;
            if since_flush >= MAX_RESPONSES_PER_FLUSH {
                break;
            }
            match rx.try_recv() {
                Ok(next) => item = next,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        if w.flush().is_err() {
            return;
        }
    }
    let _ = w.flush();
}

fn handle_request<W: Write>(w: &mut W, (req, read_at): (Request, Instant)) -> std::io::Result<()> {
    let start = Instant::now();
    let server_queue_ns = start.duration_since(read_at).as_nanos() as u64;
    let result = req.work.perform();
    let server_time_ns = start.elapsed().as_nanos() as u64;

    let resp = Response {
        id: req.id,
        server_time_ns,
        server_queue_ns,
        payload: result.unwrap_or_default(), 
    };
    framing::send(w, &resp)
}

fn main() {
    let args = WoonsocketServerOpt::parse();
    let start = Instant::now();

    let listener = TcpListener::bind(("0.0.0.0", args.port)).expect("failed to bind port");
    eprintln!("server listening on port {}", args.port);

    let served = Arc::new(AtomicU64::new(0));
    let connections = Arc::new(AtomicU64::new(0));

    {
        let served = Arc::clone(&served);
        let connections = Arc::clone(&connections);
        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        connections.fetch_add(1, Ordering::Relaxed);
                        let served = Arc::clone(&served);
                        thread::spawn(move || handle_connection(stream, served));
                    }
                    Err(e) => eprintln!("accept failed: {e}"),
                }
            }
        });
    }

    thread::sleep(Duration::from_secs(args.runtime_secs));

    let summary = format!(
        "runtime_secs={}\nelapsed_secs={:.3}\nconnections={}\nrequests_served={}\ncpus={}\n",
        args.runtime_secs,
        start.elapsed().as_secs_f64(),
        connections.load(Ordering::Relaxed),
        served.load(Ordering::Relaxed),
        thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
    );

    if let Err(e) = fs::create_dir_all(&args.outpath) {
        eprintln!("could not create outpath: {e}");
    } else {
        let path = args.outpath.join("server-summary.txt");
        match fs::File::create(&path) {
            Ok(mut f) => {
                let _ = f.write_all(summary.as_bytes());
            }
            Err(e) => eprintln!("could not write {}: {e}", path.display()),
        }
    }

    print!("{summary}");
    std::process::exit(0);
}

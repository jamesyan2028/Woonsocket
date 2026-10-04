use std::fs;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use woonsocket::framing;
use woonsocket::protocol::{Request, Response};
use woonsocket_work::args::WoonsocketServerOpt;


fn handle_connection(mut stream: TcpStream, served: Arc<AtomicU64>) {

    if let Err(e) = stream.set_nodelay(true) {
        eprintln!("set_nodelay failed: {e}");
    }

    loop {
        let req: Request = match framing::receive(&mut stream) {
            Ok(r) => r,
            Err(_) => return,
        };

        let start = Instant::now();
        let result = req.work.perform();
        let server_time_ns = start.elapsed().as_nanos() as u64;

        let resp = Response {
            id: req.id,
            server_time_ns,
            payload: result.unwrap_or_default(),
        };

        if framing::send(&mut stream, &resp).is_err() {
            return;
        }
        served.fetch_add(1, Ordering::Relaxed);
    }
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
                        // One thread per connection.
                        thread::spawn(move || handle_connection(stream, served));
                    }
                    Err(e) => eprintln!("accept failed: {e}"),
                }
            }
        });
    }


    thread::sleep(Duration::from_secs(args.runtime_secs));

    let summary = format!(
        "runtime_secs={}\nelapsed_secs={:.3}\nconnections={}\nrequests_served={}\n",
        args.runtime_secs,
        start.elapsed().as_secs_f64(),
        connections.load(Ordering::Relaxed),
        served.load(Ordering::Relaxed),
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
use clap::Parser;
use woonsocket::client::{closed_loop, open_loop};
use woonsocket_work::args::{ClientMode, WoonsocketClientOpt};

fn main() {
    let opts = WoonsocketClientOpt::parse();
    let result = match opts.mode.clone() {
        ClientMode::ClosedLoop { num_threads } => closed_loop::run(&opts, num_threads),
        ClientMode::OpenLoop { interval_us, kind } => open_loop::run(&opts, interval_us, kind),
    };
    if let Err(e) = result {
        eprintln!("client failed: {e}");
        std::process::exit(1);
    }
}

use clap::Parser;
use sam_bench::receiver::{run_receiver, ReceiverConfig};
use std::{
    process,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:7656")]
    sam: String,
    #[arg(long)]
    size: i64,
    #[arg(long, default_value_t = 42)]
    seed: i64,
    #[arg(long)]
    id: Option<String>,
    #[arg(long, default_value_t = 120)]
    timeout: u64,
}

fn main() {
    let args = Args::parse();
    if args.size == 0 {
        eprintln!("--size is required");
        process::exit(1);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let cfg = ReceiverConfig {
        sam_addr: args.sam,
        size: args.size,
        seed: args.seed,
        id: args.id.unwrap_or_else(|| format!("recv_{now}")),
        timeout: Duration::from_secs(args.timeout),
    };
    if run_receiver(cfg, &mut std::io::stdout()).is_err() {
        process::exit(1);
    }
}

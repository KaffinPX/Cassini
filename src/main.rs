mod mining;
mod pool;

use anyhow::Result;
use clap::Parser;
use mining::Mining;
use pool::Pool;
use tokio::sync::{broadcast, mpsc};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Pool api URL (will be used for fetching templates and submitting work).
    #[arg(short, long)]
    pool: String,

    /// A wallet address to be mined into.
    #[arg(short, long)]
    address: String,

    /// Number of cores to be used for mining, worker count.
    #[arg(short, long, default_value_t = 1)]
    cores: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("cassini=info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();

    info!(
        "Starting miner with {} workers over {}...",
        args.cores, args.pool
    );

    let (worker_channel, _) = broadcast::channel(16);
    let (nonce_sender, nonce_receiver) = mpsc::channel(100);
    let mining = Mining::new(worker_channel.clone(), nonce_sender);
    let pool = Pool::new(args.pool, args.address);

    let template_refresher = pool.spawn_template_refresher(worker_channel);
    let nonce_processor = pool.spawn_nonce_processor(nonce_receiver);
    let hashes_monitor = mining.spawn_monitoring();
    for _ in 0..args.cores {
        mining.spawn_worker();
    }

    tokio::signal::ctrl_c().await?;

    mining.terminate_workers();
    hashes_monitor.abort();
    template_refresher.abort();
    nonce_processor.abort();

    Ok(())
}

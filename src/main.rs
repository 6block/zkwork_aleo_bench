#[forbid(unsafe_code)]
mod prover;

use std::sync::Arc;
use structopt::StructOpt;
use tracing::{debug, error, info};

use crate::prover::Prover;

#[derive(Debug, StructOpt)]
#[structopt(name = "prover", about = "Standalone prover.", setting = structopt::clap::AppSettings::ColoredHelp)]
struct Opt {
    /// Number of parallel instances
    #[structopt(long = "parallel_num")]
    parallel_num: u16,

    /// Number of cpu threads in each thread pool
    #[structopt(long = "threads")]
    threads: u16,
}

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();
    let tracing_level = tracing::Level::INFO;
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(tracing_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("unable to set global default subscriber");

    info!("Starting prover");

    let threads = opt.parallel_num;
    let cpu_threads = opt.threads;

    let prover: Arc<Prover> = match Prover::init(threads, cpu_threads).await {
        Ok(prover) => prover,
        Err(e) => {
            error!("Unable to initialize prover: {}", e);
            std::process::exit(1);
        }
    };
    debug!("Prover initialized");
    handle_signals(prover);

    std::future::pending::<()>().await;
}

fn handle_signals(prover: Arc<Prover>) {
    tokio::task::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                let _ = prover.exit();
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                info!("Exit gracefully");
                std::process::exit(0);
            }
            Err(error) => error!("tokio::signal::ctrl_c encountered an error: {}", error),
        }
    });
}

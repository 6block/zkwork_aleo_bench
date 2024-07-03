use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    thread::sleep,
    time::Duration,
};

use ansi_term::Colour::Cyan;
use anyhow::Result;
use rand::Rng;
use rayon::{ThreadPool, ThreadPoolBuilder};
use snarkvm::{
    console,
    prelude::{Address, Network, PrivateKey, TestnetV0},
    synthesizer::VM,
    utilities::TestRng,
};
use tokio::task;
use tracing::{debug, info, trace};
type CurrentNetwork = console::network::TestnetV0;
use snarkvm::prelude::store::helpers::memory::ConsensusMemory;

pub struct Prover {
    thread_pools: Arc<Vec<Arc<ThreadPool>>>,
    terminator: Arc<AtomicBool>,
    total_proofs: Arc<AtomicU32>,
}

impl Prover {
    pub async fn init(threads: u16, cpu_threads: u16) -> Result<Arc<Self>> {
        let mut thread_pools: Vec<Arc<ThreadPool>> = Vec::new();
        let pool_count = threads;

        for index in 0..pool_count {
            let pool = ThreadPoolBuilder::new()
                .stack_size(8 * 1024 * 1024)
                .num_threads(cpu_threads as usize)
                .thread_name(move |idx| format!("ap-cpu-{}-{}", index, idx))
                .build()?;
            thread_pools.push(Arc::new(pool));
        }
        info!("Created {} prover thread pools", thread_pools.len(),);

        let terminator = Arc::new(AtomicBool::new(false));
        let prover = Arc::new(Self {
            thread_pools: Arc::new(thread_pools),
            terminator,
            total_proofs: Default::default(),
        });

        let p = prover.clone();

        let difficulty = 1024;
        task::spawn(async move {
            let epoch_hash = <CurrentNetwork as Network>::BlockHash::default();
            p.new_work(difficulty, epoch_hash, cpu_threads).await;
        });

        info!("Created prover message handler");

        let total_proofs = prover.total_proofs.clone();
        task::spawn(async move {
            fn calculate_proof_rate(now: u32, past: u32, interval: u32) -> Box<str> {
                if interval < 1 {
                    return Box::from("---");
                }
                if now <= past || past == 0 {
                    return Box::from("---");
                }
                let rate = (now - past) as f64 / (interval * 60) as f64;
                Box::from(format!("{:.2}", rate))
            }
            let mut log = VecDeque::<u32>::from(vec![0; 60]);
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                let proofs = total_proofs.load(Ordering::SeqCst);

                log.push_back(proofs);
                let m1 = *log.get(59).unwrap_or(&0);
                let m5 = *log.get(55).unwrap_or(&0);
                let m15 = *log.get(45).unwrap_or(&0);
                let m30 = *log.get(30).unwrap_or(&0);
                let m60 = log.pop_front().unwrap_or_default();
                println!(
                    "{}",
                    Cyan.normal().paint(format!(
                        "Total proofs: {} (1m: {} p/s, 5m: {} p/s, 15m: {} p/s, 30m: {} p/s, 60m: {} p/s)",
                        proofs,
                        calculate_proof_rate(proofs, m1, 1),
                        calculate_proof_rate(proofs, m5, 5),
                        calculate_proof_rate(proofs, m15, 15),
                        calculate_proof_rate(proofs, m30, 30),
                        calculate_proof_rate(proofs, m60, 60),
                    ))
                );
            }
        });
        debug!("Created proof rate calculator");

        Ok(prover)
    }

    async fn new_work(
        &self,
        share_difficulty: u64,
        epoch_hash: <CurrentNetwork as Network>::BlockHash,
        cpu_threads: u16,
    ) {
        let terminator = self.terminator.clone();
        let thread_pools = self.thread_pools.clone();
        let total_proofs = self.total_proofs.clone();
        let mut rng = TestRng::default();
        let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng).unwrap();
        let address = Address::try_from(private_key).unwrap();
        let puzzle = VM::<TestnetV0, ConsensusMemory<TestnetV0>>::new_puzzle().unwrap();

        std::thread::spawn(move || {
            sleep(Duration::from_millis(50));

            for (i, tp) in thread_pools.iter().enumerate() {
                println!("ThreadPool {:?} is {:?}", i, tp);
                let puzzle = puzzle.clone();
                let epoch_hash = epoch_hash.clone();
                let address = address.clone();
                let terminator = terminator.clone();
                let total_proofs = total_proofs.clone();
                let tp = tp.clone();
                std::thread::spawn(move || {
                    tp.install(move || {
                            loop {
                                if terminator.load(Ordering::SeqCst) {
                                    debug!("process({i}) exit.");
                                    break;
                                }
                                //for _ in 0..1 {
                                let prover_solution = match puzzle.prove(
                                    epoch_hash,
                                    address,
                                    rand::thread_rng().gen(),
                                    Some(share_difficulty),
                                ) {
                                    Ok(solution) => solution,
                                    Err(error) => {
                                        trace!("Failed to generate prover solution: {error}");
                                        total_proofs.fetch_add(1, Ordering::SeqCst);
                                        continue;
                                    }
                                };
                                let solution_target = prover_solution.target();
                                match solution_target >= share_difficulty {
                                    true => {
                                        info!("Found a Solution (Proof Target {}, Target {})",solution_target, share_difficulty);
                                    }
                                    false => debug!(
                                        "Prover solution was below the necessary proof target ({solution_target} < {share_difficulty})"
                                    ),
                                }
                                total_proofs.fetch_add(1, Ordering::SeqCst);
                                //}
                            }
                        });
                });
                sleep(Duration::from_millis(20));
            }
        });
    }

    pub fn exit(&self) {
        self.terminator.store(true, Ordering::SeqCst);
    }
}

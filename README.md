# ZKWork Aleo fine-tuning tool

A tool used to get best parameters on different cpu.

## How to build

* Clone, `git clone https://github.com/6block/zkwork_aleo_bench.git`
* Build, `cargo build --release`

Or you can download the pre-built version, `wget https://github.com/6block/zkwork_aleo_bench/releases/download/v0.0.1/prover`

## Usage

Try different values for `--parallel_num` and `--threads` to get the highest hashrate.

`./target/release/prover --parallel_num 16 --threads 1`

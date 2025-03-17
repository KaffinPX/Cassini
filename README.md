# Cassini ⛏️
### Off-node (phoebe) miner for [Neptune](https://neptune.cash).

## Features
* Supports parallel workers, simple and flat.
* Calculates and monitors hashrate per second.
* Zero fee and fully open source.
* Supports pools with phoebe protocol.

## Building
1. Install the [rust toolchain](https://rustup.rs/)
2. Clone the repository:
```bash
  git clone https://github.com/KaffinPX/Cassini
  cd cassini
```
3. Build and run Cassini:
```bash
  cargo run --release -- --pool http://localhost:8085 --address <A neptune wallet address>
```
4. For detailed help:
```bash
  cargo run -- --help
```
## Phoebe Instances
### http://45.149.206.49 **testnet**

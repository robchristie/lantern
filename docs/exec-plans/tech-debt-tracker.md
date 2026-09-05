# Tech Debt Tracker

Track recurring product or harness gaps here.

## Current Debt

- `scripts/quality-sweep.sh` currently stops at strict Clippy because
  `crates/lantern-core/src/cdp.rs` uses a single-arm `match` where Rust 1.97's
  `clippy::single_match` recommends `if let`. Standard repository validation
  remains green; address this separately from feature work that does not touch
  the CDP transport.

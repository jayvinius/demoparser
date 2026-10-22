//! Process-wide counters for user-command decoding, used by the `usercmd_audit` binary to
//! measure how many commands decode, how many fail and why. Counting is a handful of relaxed
//! atomic adds per command, so it stays on in normal parses.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Counter {
    Commands,
    Full,
    Delta,
    DeltaOk,
    DeltaNoBaseline,
    DeltaFailed,
    NumberDuplicate,
    NumberRegressed,
    NumberGap,
    StaleSkipped,
    HistoryEntries,
    HistoryNoAngles,
    HistoryNoPitch,
    HistoryNoYaw,
}

pub const COUNTERS: [Counter; 14] = [
    Counter::Commands,
    Counter::Full,
    Counter::Delta,
    Counter::DeltaOk,
    Counter::DeltaNoBaseline,
    Counter::DeltaFailed,
    Counter::NumberDuplicate,
    Counter::NumberRegressed,
    Counter::NumberGap,
    Counter::StaleSkipped,
    Counter::HistoryEntries,
    Counter::HistoryNoAngles,
    Counter::HistoryNoPitch,
    Counter::HistoryNoYaw,
];

static VALUES: [AtomicU64; COUNTERS.len()] = [const { AtomicU64::new(0) }; COUNTERS.len()];
static FAILURES: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

pub fn bump(counter: Counter) {
    add(counter, 1);
}

pub fn add(counter: Counter, n: u64) {
    VALUES[counter as usize].fetch_add(n, Ordering::Relaxed);
}

pub fn record_failure(reason: String) {
    bump(Counter::DeltaFailed);
    if let Ok(mut map) = FAILURES.lock() {
        *map.entry(reason).or_insert(0) += 1;
    }
}

pub fn reset() {
    for v in &VALUES {
        v.store(0, Ordering::Relaxed);
    }
    if let Ok(mut map) = FAILURES.lock() {
        map.clear();
    }
}

pub fn get(counter: Counter) -> u64 {
    VALUES[counter as usize].load(Ordering::Relaxed)
}

pub fn snapshot() -> (Vec<(Counter, u64)>, BTreeMap<String, u64>) {
    let counts = COUNTERS.iter().map(|&c| (c, get(c))).collect();
    let failures = FAILURES.lock().map(|m| m.clone()).unwrap_or_default();
    (counts, failures)
}

//! Tier 1: loading a config file with many duplicate keys or sections scales
//! linearly, as C's uthash does (it prepends to a bucket chain in O(1)).
//!
//! Identical keys all land in one bucket, uthash stops expanding after two
//! ineffective expands, and the chain grows to the number of duplicates. A
//! model that pays O(chain) per insert turns a few hundred kilobytes of
//! `k=` lines into a load that takes seconds to minutes.
//!
//! The test compares one load of `LARGE` entries with `COPIES` loads of
//! `SMALL` entries, where `LARGE = COPIES * SMALL`. Both phases do the same
//! linear work, so the ratio is about 1 for linear work and about `COPIES`
//! (32) for quadratic work, on any machine. Both phases also run for about
//! the same time, so a busy host slows both by a similar share.
//!
//! An earlier form timed one load of 10000 entries against one of 80000 and
//! failed on the macOS CI runner with ratios of 24 and 28 against a bound of
//! 24 (#4835). Locally, on efficiency cores under load, its ratio spread from
//! 7 to 21 while this ratio stays near 1. A large load that is also slower per
//! entry (more memory) adds to both forms; the wide gap between 1 and
//! `COPIES` leaves room for it.

use std::time::{Duration, Instant};

use obs_util::config_file::Config;

const SMALL: usize = 2_500;
const COPIES: usize = 32;
const LARGE: usize = COPIES * SMALL;
/// Rounds of both phases, interleaved, so both sample the same load.
const ROUNDS: usize = 3;
/// Linear work gives about 1, quadratic about `COPIES` (32).
const MAX_RATIO: f64 = 8.0;

fn repeated_keys(n: usize) -> Vec<u8> {
    let mut t = String::with_capacity(n * 12 + 4);
    t.push_str("[s]\n");
    for i in 0..n {
        t.push_str(&format!("k=v{i}\n"));
    }
    t.into_bytes()
}

fn repeated_sections(n: usize) -> Vec<u8> {
    let mut t = String::with_capacity(n * 14);
    for i in 0..n {
        t.push_str(&format!("[s]\nk=v{i}\n"));
    }
    t.into_bytes()
}

/// Loads, checks and drops `text` `copies` times and returns the time of all
/// of it. Both phases time the same steps, and only one config is alive at a
/// time.
fn timed_loads(text: &[u8], copies: usize, check: impl Fn(&Config)) -> Duration {
    let start = Instant::now();
    for _ in 0..copies {
        check(&Config::open_string(text));
    }
    start.elapsed()
}

fn assert_linear(name: &str, build: fn(usize) -> Vec<u8>, want: fn(usize) -> Option<String>) {
    let small = build(SMALL);
    let large = build(LARGE);
    let check = |n: usize| {
        let want = want(n).map(String::into_bytes);
        move |c: &Config| assert_eq!(c.get_string(b"s", b"k"), want, "{name} {n}")
    };
    // Best of each phase over interleaved rounds: a noisy neighbour can only
    // make a phase slower.
    let mut t_small = Duration::MAX;
    let mut t_large = Duration::MAX;
    for _ in 0..ROUNDS {
        t_small = t_small.min(timed_loads(&small, COPIES, check(SMALL)));
        t_large = t_large.min(timed_loads(&large, 1, check(LARGE)));
    }
    // Floor the small time so timer resolution cannot inflate the ratio.
    let ratio = t_large.as_secs_f64() / t_small.as_secs_f64().max(1e-3);
    assert!(
        ratio < MAX_RATIO,
        "{name}: 1 load of {LARGE} entries took {t_large:?}, {COPIES} loads of {SMALL} \
         took {t_small:?}; ratio {ratio:.2} (linear is about 1, quadratic about {COPIES})"
    );
}

/// Duplicate keys in one section.
#[test]
fn duplicate_keys_load_in_linear_time() {
    assert_linear("keys", repeated_keys, |n| Some(format!("v{}", n - 1)));
}

/// Duplicate `[s]` headers.
#[test]
fn duplicate_sections_load_in_linear_time() {
    assert_linear("sections", repeated_sections, |n| {
        Some(format!("v{}", n - 1))
    });
}

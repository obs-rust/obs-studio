//! Tier 1: loading a config file with many duplicate keys or sections scales
//! linearly, as C's uthash does (it prepends to a bucket chain in O(1)).
//!
//! Identical keys all land in one bucket, uthash stops expanding after two
//! ineffective expands, and the chain grows to the number of duplicates. A
//! model that pays O(chain) per insert turns a few hundred kilobytes of
//! `k=` lines into a load that takes seconds to minutes. The test times the
//! load at two sizes and checks the ratio, which stays near the size ratio for
//! linear work and near its square for quadratic work, so it does not depend
//! on how fast the machine is.

use std::time::{Duration, Instant};

use obs_util::config_file::Config;

const SMALL: usize = 10_000;
const LARGE: usize = 8 * SMALL;
/// Linear work gives about 8, quadratic about 64.
const MAX_RATIO: f64 = 24.0;

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

/// Best of three loads, so a noisy neighbour on CI only makes it slower.
fn best_load(text: &[u8], check: impl Fn(&Config)) -> Duration {
    (0..3)
        .map(|_| {
            let start = Instant::now();
            let c = Config::open_string(text);
            let took = start.elapsed();
            check(&c);
            took
        })
        .min()
        .unwrap()
}

fn assert_linear(name: &str, build: fn(usize) -> Vec<u8>, want: fn(usize) -> Option<String>) {
    let small = build(SMALL);
    let large = build(LARGE);
    let t_small = best_load(&small, |c| {
        assert_eq!(
            c.get_string(b"s", b"k"),
            want(SMALL).map(String::into_bytes),
            "{name} {SMALL}"
        );
    });
    let t_large = best_load(&large, |c| {
        assert_eq!(
            c.get_string(b"s", b"k"),
            want(LARGE).map(String::into_bytes),
            "{name} {LARGE}"
        );
    });
    // Floor the small time so timer resolution cannot inflate the ratio.
    let ratio = t_large.as_secs_f64() / t_small.as_secs_f64().max(1e-3);
    assert!(
        ratio < MAX_RATIO,
        "{name}: {LARGE} entries took {t_large:?}, {SMALL} took {t_small:?}; \
         ratio {ratio:.1} for 8x the input (linear is about 8, quadratic about 64)"
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

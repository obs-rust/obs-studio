//! Walking an object with `first`/`next` (`obs_data_first` and
//! `obs_data_item_next`) is linear, like the C original's uthash `hh.next`,
//! and keeps its order when items are added or erased during the walk.

use std::time::{Duration, Instant};

use obs_data::Data;

fn key(i: usize) -> String {
    format!("k{i}")
}

fn names(data: &Data) -> Vec<String> {
    let mut out = Vec::new();
    let mut item = data.first();
    while let Some(current) = item {
        out.push(current.name());
        item = current.next();
    }
    out
}

/// Builds an object of `n` items and walks it once, returning the build and
/// walk times and the number of items the walk visited.
fn build_and_walk(n: usize) -> (Duration, Duration, usize) {
    let start = Instant::now();
    let data = Data::new();
    for i in 0..n {
        data.set_int(&key(i), i as i64);
    }
    let build = start.elapsed();

    let start = Instant::now();
    let mut visited = 0;
    let mut item = data.first();
    while let Some(current) = item {
        visited += 1;
        item = current.next();
    }
    let walk = start.elapsed();
    (build, walk, visited)
}

/// A full walk costs about as much as building the object, not n times as
/// much. The bound is relative to the build time on the same machine, so a
/// slow runner slows both sides alike. A quadratic walk of 40 000 items does
/// about 800 million pointer compares and misses the bound by far; a linear
/// one does 40 000 steps.
#[test]
fn full_walk_is_linear() {
    const N: usize = 40_000;
    const K: u32 = 4;
    // Best of three, so one descheduled run does not decide the result.
    let mut best_ratio = f64::INFINITY;
    let mut last = (Duration::ZERO, Duration::ZERO);
    for _ in 0..3 {
        let (build, walk, visited) = build_and_walk(N);
        assert_eq!(visited, N);
        let ratio = walk.as_secs_f64() / build.as_secs_f64().max(1e-9);
        if ratio < best_ratio {
            best_ratio = ratio;
            last = (build, walk);
        }
        if ratio < f64::from(K) {
            break;
        }
    }
    assert!(
        best_ratio < f64::from(K),
        "walking {N} items took {:?}, {best_ratio:.1}x the {:?} it took to build them (limit {K}x)",
        last.1,
        last.0,
    );
}

/// Erasing the next item skips it, erasing an earlier item does not disturb
/// the walk, erasing the current item ends it, and a key set during the walk
/// is appended and reached at the end.
#[test]
fn walk_survives_changes() {
    let n = 40;
    let data = Data::new();
    for i in 0..n {
        data.set_int(&key(i), i as i64);
    }

    let mut seen = Vec::new();
    let mut item = data.first();
    while let Some(current) = item {
        let name = current.name();
        seen.push(name.clone());
        match name.as_str() {
            // The next item goes, so the walk skips it.
            "k5" => data.erase("k6"),
            // Earlier items go, so later items move down.
            "k10" => {
                data.erase("k0");
                data.erase("k1");
                data.erase("k9");
            }
            // A new key goes on the end.
            "k20" => data.set_int("late", 1),
            // An existing key keeps its place.
            "k21" => data.set_int("k30", 99),
            _ => {}
        }
        item = current.next();
    }

    let mut expected: Vec<String> = (0..n).filter(|i| *i != 6).map(key).collect();
    expected.push("late".to_owned());
    assert_eq!(seen, expected);

    let mut left: Vec<String> = (0..n)
        .filter(|i| ![0, 1, 6, 9].contains(i))
        .map(key)
        .collect();
    left.push("late".to_owned());
    assert_eq!(names(&data), left);
    assert_eq!(data.get_int("k30"), 99);

    // The current item erased: its handle stays usable but the walk ends.
    let mut item = data.first();
    for _ in 0..3 {
        item = item.unwrap().next();
    }
    let current = item.unwrap();
    assert_eq!(current.name(), "k5");
    data.erase("k5");
    assert_eq!(current.name(), "k5");
    assert!(current.next().is_none());

    // The same holds after many erases ahead of the walk position.
    let data = Data::new();
    for i in 0..200 {
        data.set_int(&key(i), i as i64);
    }
    let mut item = data.first();
    for _ in 0..150 {
        item = item.unwrap().next();
    }
    for i in 0..100 {
        data.erase(&key(i));
    }
    let mut rest = Vec::new();
    while let Some(current) = item {
        rest.push(current.name());
        item = current.next();
    }
    assert_eq!(rest, (150..200).map(key).collect::<Vec<_>>());
}

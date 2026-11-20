//! os_get_free_disk_space answers within its deadline on a hung volume (#1345).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use obs_util::bounded_free_space::BoundedFreeSpace;

const DEADLINE: Duration = Duration::from_millis(100);
/// Slack for a loaded CI runner on top of the deadline.
const LATE: Duration = Duration::from_millis(2000);

/// A query that answers from `answers`, blocking while the channel is empty,
/// and counts its calls.
fn gated() -> (BoundedFreeSpace, mpsc::Sender<u64>, Arc<AtomicUsize>) {
    let (tx, rx) = mpsc::channel::<u64>();
    let rx = Arc::new(Mutex::new(rx));
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let query = Arc::new(move |_: &[u8]| {
        counted.fetch_add(1, Ordering::SeqCst);
        rx.lock().unwrap().recv().unwrap_or(0)
    });
    (BoundedFreeSpace::new(query, DEADLINE), tx, calls)
}

fn timed(space: &BoundedFreeSpace, path: &[u8]) -> (u64, Duration) {
    let start = Instant::now();
    (space.get(path), start.elapsed())
}

#[test]
fn a_responsive_volume_answers_its_value() {
    let space = BoundedFreeSpace::new(Arc::new(|path: &[u8]| path.len() as u64 * 1000), DEADLINE);
    assert_eq!(space.get(b"/rec"), 4000);
    assert_eq!(space.get(b"/recordings"), 11000);
}

#[test]
fn a_hung_volume_answers_zero_within_the_deadline_before_any_value() {
    let (space, tx, _) = gated();
    let (value, took) = timed(&space, b"/nas");
    assert_eq!(value, 0);
    assert!(took < LATE, "waited {took:?}");
    drop(tx);
}

#[test]
fn a_hung_volume_answers_its_last_value() {
    let (space, tx, _) = gated();
    tx.send(5_000).unwrap();
    assert_eq!(space.get(b"/nas"), 5_000);
    let (value, took) = timed(&space, b"/nas");
    assert_eq!(value, 5_000);
    assert!(took < LATE, "waited {took:?}");
    drop(tx);
}

#[test]
fn calls_while_a_query_hangs_start_no_new_query_and_do_not_wait() {
    let (space, tx, calls) = gated();
    let _ = space.get(b"/nas");
    for _ in 0..20 {
        let (_, took) = timed(&space, b"/nas");
        assert!(took < DEADLINE, "an in-flight path waited {took:?}");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1, "one helper per hung path");
    drop(tx);
}

#[test]
fn a_volume_that_recovers_answers_fresh_values_again() {
    let (space, tx, calls) = gated();
    let _ = space.get(b"/nas");
    tx.send(7_000).unwrap();
    let deadline = Instant::now() + LATE;
    let mut fresh = 0;
    while Instant::now() < deadline {
        tx.send(9_000).unwrap();
        fresh = space.get(b"/nas");
        if fresh == 9_000 {
            break;
        }
    }
    assert_eq!(fresh, 9_000);
    assert!(calls.load(Ordering::SeqCst) >= 2);
}

#[test]
fn paths_do_not_block_each_other() {
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Mutex::new(release_rx);
    let (started_tx, started_rx) = mpsc::channel();
    let query = Arc::new(move |path: &[u8]| {
        if path == b"/nas" {
            started_tx.send(()).unwrap();
            let _ = release_rx.lock().unwrap().recv();
        }
        42
    });
    // #5299: the 100 ms query deadline also includes helper scheduling time.
    // This test checks path isolation; the tests above check bounded waits.
    let watchdog = Duration::from_secs(10);
    let space = Arc::new(BoundedFreeSpace::new(query, Duration::from_secs(60)));
    let nas_space = space.clone();
    let nas = std::thread::spawn(move || nas_space.get(b"/nas"));
    let started = started_rx.recv_timeout(watchdog);

    let (home_tx, home_rx) = mpsc::channel();
    let home = std::thread::spawn(move || home_tx.send(space.get(b"/home")).unwrap());
    // /home must answer while /nas is still blocked. The timeout is only a
    // deadlock watchdog, not an assertion about how quickly a helper runs.
    let home_value = home_rx.recv_timeout(watchdog);

    // Unblock and join both callers even when isolation fails.
    drop(release_tx);
    let nas_value = nas.join().unwrap();
    home.join().unwrap();
    assert_eq!(started, Ok(()), "the /nas query never started");
    assert_eq!(
        home_value,
        Ok(42),
        "/home did not answer while /nas was blocked"
    );
    assert_eq!(nas_value, 42);
}

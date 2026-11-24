//! Tier 1 mirrors of test_source_showing_active.c.
use obs_source::showing_active::{Counter, nonzero};
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

/// One predicate over a model source holding `[activate_refs, show_refs]`,
/// recording the counters read and the names logged.
fn ask(
    counter: Counter,
    refs: Option<[i64; 2]>,
    reads: &RefCell<Vec<Counter>>,
    logs: &RefCell<Vec<&'static str>>,
) -> bool {
    let source = refs.map(|[activate, show]| {
        move |read: Counter| {
            reads.borrow_mut().push(read);
            match read {
                Counter::Activate => activate,
                Counter::Show => show,
            }
        }
    });
    nonzero(counter, source, |name| {
        logs.borrow_mut().push(name.to_str().unwrap());
    })
}

/// null_source_is_false_and_names_the_caller: NULL is false, read nothing,
/// and logs once under the name of the function that was called.
#[test]
fn null_source_is_false_and_names_the_caller() {
    let (reads, logs) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    assert!(!ask(Counter::Activate, None, &reads, &logs));
    assert_eq!(*logs.borrow(), ["obs_source_active"]);
    assert!(!ask(Counter::Show, None, &reads, &logs));
    assert_eq!(*logs.borrow(), ["obs_source_active", "obs_source_showing"]);
    assert!(reads.borrow().is_empty());
    assert_eq!(Counter::Activate.function(), c"obs_source_active");
    assert_eq!(Counter::Show.function(), c"obs_source_showing");
}

/// each_reads_only_its_own_counter: every pair of counter values gives two
/// independent "nonzero" answers, each from one read of its own counter,
/// with no log. The edges include both C `long` widths (LLP64 and LP64).
#[test]
fn each_reads_only_its_own_counter() {
    let edges = [
        0,
        1,
        2,
        -1,
        0x5a5a,
        i64::from(i32::MAX),
        i64::from(i32::MIN),
        i64::MAX,
        i64::MIN,
    ];
    let (reads, logs) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    for activate in edges {
        for show in edges {
            for _repeat in 0..2 {
                reads.borrow_mut().clear();
                let refs = Some([activate, show]);
                assert_eq!(ask(Counter::Activate, refs, &reads, &logs), activate != 0);
                assert_eq!(ask(Counter::Show, refs, &reads, &logs), show != 0);
                assert_eq!(*reads.borrow(), [Counter::Activate, Counter::Show]);
            }
        }
    }
    assert!(logs.borrow().is_empty());
}

/// Every single bit of either counter is nonzero on its own.
#[test]
fn single_bits_are_nonzero() {
    let (reads, logs) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    for bit in 0..64 {
        let value = 1i64 << bit;
        assert!(ask(Counter::Activate, Some([value, 0]), &reads, &logs));
        assert!(!ask(Counter::Show, Some([value, 0]), &reads, &logs));
        assert!(!ask(Counter::Activate, Some([0, value]), &reads, &logs));
        assert!(ask(Counter::Show, Some([0, value]), &reads, &logs));
    }
    assert!(logs.borrow().is_empty());
}

const FLIPS: u32 = 20_000;

/// reads_while_another_thread_changes_the_counts: a second thread raises
/// and lowers both counts while this one asks; each answer is read through
/// the caller's atomic load, the counts end balanced and both answers false.
#[test]
fn reads_while_another_thread_changes_the_counts() {
    let refs = Arc::new([AtomicI64::new(0), AtomicI64::new(0)]);
    let started = Arc::new(AtomicBool::new(false));
    let flipper = {
        let (refs, started) = (Arc::clone(&refs), Arc::clone(&started));
        std::thread::spawn(move || {
            started.store(true, Ordering::SeqCst);
            for _ in 0..FLIPS {
                refs[0].fetch_add(1, Ordering::SeqCst);
                refs[1].fetch_add(1, Ordering::SeqCst);
                refs[1].fetch_sub(1, Ordering::SeqCst);
                refs[0].fetch_sub(1, Ordering::SeqCst);
            }
        })
    };
    while !started.load(Ordering::SeqCst) {
        std::hint::spin_loop();
    }
    let logs = Cell::new(0u32);
    let load = |counter: Counter| match counter {
        Counter::Activate => refs[0].load(Ordering::SeqCst),
        Counter::Show => refs[1].load(Ordering::SeqCst),
    };
    let log = |_: &'static core::ffi::CStr| logs.set(logs.get() + 1);
    let (mut active_seen, mut showing_seen) = (0u32, 0u32);
    for _ in 0..FLIPS {
        active_seen += u32::from(nonzero(Counter::Activate, Some(load), log));
        showing_seen += u32::from(nonzero(Counter::Show, Some(load), log));
    }
    flipper.join().unwrap();
    assert!(active_seen <= FLIPS);
    assert!(showing_seen <= FLIPS);
    assert_eq!(refs[0].load(Ordering::SeqCst), 0);
    assert_eq!(refs[1].load(Ordering::SeqCst), 0);
    assert!(!nonzero(Counter::Activate, Some(load), log));
    assert!(!nonzero(Counter::Show, Some(load), log));
    assert_eq!(logs.get(), 0);
}

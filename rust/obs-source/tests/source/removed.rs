//! Tier 1 mirrors of test_source_removed.c.
use obs_source::removed::{FUNCTION, removed};
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The predicate over a model source holding `flag` (`None` is NULL),
/// counting the reads and recording the names logged.
fn ask(flag: Option<bool>, reads: &Cell<u32>, logs: &RefCell<Vec<&'static str>>) -> bool {
    let source = flag.map(|flag| {
        move || {
            reads.set(reads.get() + 1);
            flag
        }
    });
    removed(source, |name| {
        logs.borrow_mut().push(name.to_str().unwrap())
    })
}

/// null_source_is_removed_and_names_the_caller: NULL is true (a missing
/// source counts as removed), reads nothing and logs once under
/// obs_source_removed.
#[test]
fn null_source_is_removed_and_names_the_caller() {
    let (reads, logs) = (Cell::new(0), RefCell::new(Vec::new()));
    assert!(ask(None, &reads, &logs));
    assert_eq!(*logs.borrow(), ["obs_source_removed"]);
    assert_eq!(reads.get(), 0);
    assert_eq!(FUNCTION, c"obs_source_removed");
}

/// reads_only_the_removed_flag: the answer is the flag, read once per call,
/// twice in a row, with no log.
#[test]
fn reads_only_the_removed_flag() {
    let (reads, logs) = (Cell::new(0), RefCell::new(Vec::new()));
    for flag in [false, true] {
        for repeat in 1..=2 {
            reads.set(0);
            assert_eq!(ask(Some(flag), &reads, &logs), flag, "repeat {repeat}");
            assert_eq!(reads.get(), 1);
        }
    }
    assert!(logs.borrow().is_empty());
}

const FLIPS: u32 = 20_000;

/// reads_while_another_thread_changes_the_flag: a second thread sets and
/// clears the flag, ending set, while this one asks through the caller's
/// atomic load; the last value written is what a later read sees.
#[test]
fn reads_while_another_thread_changes_the_flag() {
    let flag = Arc::new(AtomicBool::new(false));
    let started = Arc::new(AtomicBool::new(false));
    let flipper = {
        let (flag, started) = (Arc::clone(&flag), Arc::clone(&started));
        std::thread::spawn(move || {
            started.store(true, Ordering::SeqCst);
            for _ in 0..FLIPS {
                flag.store(true, Ordering::SeqCst);
                flag.store(false, Ordering::SeqCst);
            }
            flag.store(true, Ordering::SeqCst);
        })
    };
    while !started.load(Ordering::SeqCst) {
        std::hint::spin_loop();
    }
    let logs = Cell::new(0u32);
    let log = |_: &'static core::ffi::CStr| logs.set(logs.get() + 1);
    let load = || flag.load(Ordering::SeqCst);
    let mut seen = 0u32;
    for _ in 0..FLIPS {
        seen += u32::from(removed(Some(load), log));
    }
    flipper.join().unwrap();
    assert!(seen <= FLIPS);
    assert!(removed(Some(load), log));
    assert_eq!(logs.get(), 0);
}

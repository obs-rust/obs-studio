//! Tier 1 mirror of `test/cmocka/test_signal.c`, plus edge cases.
//!
//! Callbacks are [`CallbackKey`] values; each test runs them through a
//! dispatcher closure. `bnum_allocs`, the log texts, and a null handler
//! stay with the cmocka test.

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use obs_callback::signal::{AddResult, CallbackKey, SignalHandler, remove_current};

// Callback "functions" by role. `data` is the id the test records.
const ON_SIGNAL: usize = 1;
const REMOVE_SELF: usize = 2;
const DISCONNECT_OTHER: usize = 3;
const CONNECT_OTHER: usize = 4;
const NESTED_THEN_REMOVE: usize = 5;
const NESTED: usize = 6;
const RECURSE_ONCE: usize = 7;
const ON_GLOBAL: usize = 10;
const REMOVE_SELF_GLOBAL: usize = 11;
const DISCONNECT_OWN_GLOBAL: usize = 12;
const CONNECT_MANY: usize = 13;
const CONNECT_MANY_GLOBALS: usize = 14;
const NESTED_ONCE_THEN_REMOVE_GLOBAL: usize = 15;
const DISCONNECT_OTHER_GLOBAL_THEN_REMOVE: usize = 16;

fn key(func: usize, data: usize) -> CallbackKey {
    CallbackKey { func, data }
}

thread_local! {
    /// `recursive_remove_test`'s depth.
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

fn handler_with(decl: &str) -> SignalHandler {
    let handler = SignalHandler::new();
    assert_eq!(handler.add(decl.as_bytes()), AddResult::Added);
    handler
}

/// Runs a signal with the roles above. `seen` gets each callback's id, and
/// a global callback's id plus the signal name. `other` is the id the
/// connect/disconnect roles act on. True when the pass gave back the last
/// reference.
fn fire(
    handler: &SignalHandler,
    signal: &str,
    seen: &RefCell<Vec<(usize, String)>>,
    other: usize,
) -> bool {
    let call = |k: CallbackKey| {
        seen.borrow_mut().push((k.data, String::new()));
        match k.func {
            REMOVE_SELF => remove_current(),
            DISCONNECT_OTHER => {
                handler.disconnect(b"ev", key(ON_SIGNAL, other));
            }
            CONNECT_OTHER => {
                handler.connect(b"ev", key(ON_SIGNAL, other), false);
            }
            CONNECT_MANY => {
                for i in 0..8 {
                    handler.connect(b"ev", key(ON_SIGNAL, 100 + i), false);
                }
                remove_current();
            }
            NESTED_THEN_REMOVE => {
                fire(handler, "inner", seen, other);
                // The nested pass restored this callback as the current
                // one, so this removes it.
                remove_current();
            }
            NESTED => {
                fire(handler, "inner", seen, other);
            }
            RECURSE_ONCE if DEPTH.get() == 0 => {
                DEPTH.set(1);
                fire(handler, signal, seen, other);
                DEPTH.set(0);
            }
            _ => {}
        }
    };
    let call_global = |k: CallbackKey| {
        seen.borrow_mut().push((k.data, signal.to_owned()));
        match k.func {
            REMOVE_SELF_GLOBAL => remove_current(),
            DISCONNECT_OWN_GLOBAL => handler.disconnect_global(k),
            CONNECT_MANY_GLOBALS => {
                for i in 0..8 {
                    handler.connect_global(key(ON_GLOBAL, 100 + i));
                }
                remove_current();
            }
            NESTED_ONCE_THEN_REMOVE_GLOBAL => {
                if DEPTH.get() == 0 {
                    DEPTH.set(1);
                    fire(handler, signal, seen, other);
                    DEPTH.set(0);
                }
                remove_current();
            }
            DISCONNECT_OTHER_GLOBAL_THEN_REMOVE => {
                handler.disconnect_global(key(ON_GLOBAL, other));
                remove_current();
            }
            _ => {}
        }
    };
    handler.signal(signal.as_bytes(), call, call_global)
}

fn ids(seen: &RefCell<Vec<(usize, String)>>) -> Vec<usize> {
    seen.borrow().iter().map(|(id, _)| *id).collect()
}

/// `create_destroy_test`.
#[test]
fn create_destroy_test() {
    let handler = SignalHandler::new();
    assert_eq!(handler.refs(), 1);
    assert!(handler.release());
}

/// `add_test`.
#[test]
fn add_test() {
    let handler = SignalHandler::new();
    assert_eq!(handler.add(b"void ev(int n)"), AddResult::Added);
    // A second declaration of the same name is refused.
    assert_eq!(
        handler.add(b"void ev(string s)"),
        AddResult::Exists(b"ev".to_vec())
    );
    // An invalid declaration carries decl's warning.
    match handler.add(b"void (int n)") {
        AddResult::Invalid(parse) => assert!(parse.warning(b"void (int n)").is_some()),
        other => panic!("{other:?}"),
    }
    // The first declaration is the one kept.
    let decl = handler.decl(b"ev").expect("ev exists");
    assert_eq!(decl.params[0].name, b"n");
}

/// `connect_signal_test`.
#[test]
fn connect_signal_test() {
    let handler = handler_with("void ev(int n)");
    let seen = RefCell::new(Vec::new());
    assert!(handler.connect(b"ev", key(ON_SIGNAL, 1), false));
    assert!(handler.connect(b"ev", key(ON_SIGNAL, 2), false));
    // The same callback and data again is not added twice.
    assert!(handler.connect(b"ev", key(ON_SIGNAL, 1), false));

    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [1, 2]);

    // An unknown signal calls nothing.
    fire(&handler, "nope", &seen, 0);
    assert_eq!(ids(&seen), [1, 2]);
}

/// `connect_unknown_test`: the shim logs the warning.
#[test]
fn connect_unknown_test() {
    let handler = handler_with("void ev()");
    assert!(!handler.connect(b"nope", key(ON_SIGNAL, 1), false));
}

/// `disconnect_test`.
#[test]
fn disconnect_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(ON_SIGNAL, 1), false);
    handler.connect(b"ev", key(ON_SIGNAL, 2), false);
    assert!(!handler.disconnect(b"ev", key(ON_SIGNAL, 1)));
    // Not connected, or an unknown signal: nothing happens.
    assert!(!handler.disconnect(b"ev", key(ON_SIGNAL, 1)));
    assert!(!handler.disconnect(b"nope", key(ON_SIGNAL, 2)));

    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [2]);
}

/// `disconnect_during_signal_test`.
#[test]
fn disconnect_during_signal_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(DISCONNECT_OTHER, 1), false);
    handler.connect(b"ev", key(ON_SIGNAL, 2), false);

    // A disconnect while signalling only marks the callback; a marked
    // callback later in the list is skipped in the same pass.
    fire(&handler, "ev", &seen, 2);
    assert_eq!(ids(&seen), [1]);
    fire(&handler, "ev", &seen, 2);
    assert_eq!(ids(&seen), [1, 1]);
}

/// `remove_current_test`.
#[test]
fn remove_current_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    // Outside a signal there is no current callback.
    remove_current();

    handler.connect(b"ev", key(REMOVE_SELF, 1), false);
    handler.connect(b"ev", key(ON_SIGNAL, 2), false);
    fire(&handler, "ev", &seen, 0);
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [1, 2, 2]);
}

/// `connect_during_signal_test`.
#[test]
fn connect_during_signal_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(CONNECT_OTHER, 1), false);

    // characterized, not endorsed: a callback connected while signalling
    // is called in the same pass.
    fire(&handler, "ev", &seen, 2);
    assert_eq!(ids(&seen), [1, 2]);
}

/// `nested_signal_test`.
#[test]
fn nested_signal_test() {
    let handler = handler_with("void ev()");
    assert_eq!(handler.add(b"void inner()"), AddResult::Added);
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(NESTED_THEN_REMOVE, 1), false);
    handler.connect(b"inner", key(ON_SIGNAL, 2), false);

    fire(&handler, "ev", &seen, 0);
    fire(&handler, "ev", &seen, 0);
    // The first callback removed itself after the nested signal.
    assert_eq!(ids(&seen), [1, 2]);

    // inner is still connected.
    fire(&handler, "inner", &seen, 0);
    assert_eq!(ids(&seen), [1, 2, 2]);
}

/// `keep_ref_test`.
#[test]
fn keep_ref_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    // With keep_ref the same pair is added again; each holds a reference.
    handler.connect(b"ev", key(ON_SIGNAL, 1), true);
    handler.connect(b"ev", key(ON_SIGNAL, 1), true);
    assert_eq!(handler.refs(), 3);
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen).len(), 2);

    // The creator's reference goes; the two callback references remain.
    assert!(!handler.release());
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen).len(), 4);

    // Each disconnect drops one reference. The last one reports it.
    assert!(!handler.disconnect(b"ev", key(ON_SIGNAL, 1)));
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen).len(), 5);
    assert!(handler.disconnect(b"ev", key(ON_SIGNAL, 1)));
}

/// `keep_ref_remove_current_test`.
#[test]
fn keep_ref_remove_current_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(REMOVE_SELF, 1), true);
    assert_eq!(handler.refs(), 2);
    // The reference comes back after the pass, without a destroy.
    fire(&handler, "ev", &seen, 0);
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [1]);
    assert_eq!(handler.refs(), 1);
    assert!(handler.release());
}

/// `keep_ref_remove_current_last_ref_test`.
#[test]
fn keep_ref_remove_current_last_ref_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(REMOVE_SELF, 1), true);
    // The creator's reference goes first. The reference the callback gives
    // back after it removes itself is then the last one.
    assert!(!handler.release());
    assert!(fire(&handler, "ev", &seen, 0));
    assert_eq!(ids(&seen), [1]);
    assert_eq!(handler.refs(), 0);
}

/// `recursive_remove_test`.
#[test]
fn recursive_remove_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(REMOVE_SELF, 1), false);
    handler.connect(b"ev", key(RECURSE_ONCE, 2), false);
    handler.connect(b"ev", key(ON_SIGNAL, 3), false);

    // The inner pass does not erase 1 while the outer pass walks the list,
    // so the outer pass still reaches 3: outer 1, outer 2, inner 2,
    // inner 3, outer 3.
    fire(&handler, "ev", &seen, 0);
    assert_eq!(DEPTH.get(), 0);
    assert_eq!(ids(&seen), [1, 2, 2, 3, 3]);

    // After the outermost pass, 1 is gone.
    seen.borrow_mut().clear();
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [2, 2, 3, 3]);
}

/// `connect_then_remove_current_test`.
#[test]
fn connect_then_remove_current_test() {
    // The callback connects enough callbacks to move the array, then
    // removes itself. The removal marks its own entry. The new callbacks
    // run in the same pass and stay.
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(CONNECT_MANY, 1), false);
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [1, 100, 101, 102, 103, 104, 105, 106, 107]);
    seen.borrow_mut().clear();
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [100, 101, 102, 103, 104, 105, 106, 107]);
}

/// `global_test`.
#[test]
fn global_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(ON_SIGNAL, 1), false);
    handler.connect_global(key(ON_GLOBAL, 9));
    // The same callback and data again is not added twice.
    handler.connect_global(key(ON_GLOBAL, 9));

    // Global callbacks run after the signal's own, with its name.
    fire(&handler, "ev", &seen, 0);
    assert_eq!(*seen.borrow(), [(1, String::new()), (9, "ev".to_owned())]);

    // characterized, not endorsed: an unknown signal returns before the
    // global callbacks.
    fire(&handler, "nope", &seen, 0);
    assert_eq!(ids(&seen), [1, 9]);
}

/// `global_remove_test`.
#[test]
fn global_remove_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect_global(key(REMOVE_SELF_GLOBAL, 7));
    // Found by callback and data while it runs, so it is removed.
    handler.connect_global(key(DISCONNECT_OWN_GLOBAL, 8));

    fire(&handler, "ev", &seen, 0);
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [7, 8]);

    // A disconnect outside a signal also finds the entry.
    handler.connect_global(key(ON_GLOBAL, 9));
    handler.disconnect_global(key(ON_GLOBAL, 9));
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [7, 8]);
}

/// `global_connect_then_remove_current_test`.
#[test]
fn global_connect_then_remove_current_test() {
    // The global callback connects enough global callbacks to move the
    // list, then removes itself. The removal marks its own entry. The new
    // global callbacks run in the same pass and stay.
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect_global(key(CONNECT_MANY_GLOBALS, 1));
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [1, 100, 101, 102, 103, 104, 105, 106, 107]);
    seen.borrow_mut().clear();
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [100, 101, 102, 103, 104, 105, 106, 107]);
}

/// `global_nested_pass_test`.
#[test]
fn global_nested_pass_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect_global(key(REMOVE_SELF_GLOBAL, 1));
    handler.connect_global(key(NESTED_ONCE_THEN_REMOVE_GLOBAL, 2));
    handler.connect_global(key(ON_GLOBAL, 3));

    // The inner pass does not erase 1 while the outer pass walks the
    // list, so 2 removes itself, not 3, and the outer pass still reaches
    // 3: outer 1, outer 2, inner 2, inner 3, outer 3.
    fire(&handler, "ev", &seen, 0);
    assert_eq!(DEPTH.get(), 0);
    assert_eq!(ids(&seen), [1, 2, 2, 3, 3]);

    // After the outermost pass, 1 and 2 are gone.
    seen.borrow_mut().clear();
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [3]);
}

/// `global_disconnect_during_signal_test`.
#[test]
fn global_disconnect_during_signal_test() {
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect_global(key(ON_GLOBAL, 1));
    handler.connect_global(key(DISCONNECT_OTHER_GLOBAL_THEN_REMOVE, 2));
    handler.connect_global(key(ON_GLOBAL, 3));

    // 2 disconnects 1, which already ran, then removes itself. 1 is not
    // erased while the pass walks the list, so 2 removes itself, not 3,
    // and the pass still reaches 3.
    fire(&handler, "ev", &seen, 1);
    assert_eq!(ids(&seen), [1, 2, 3]);

    // 1 and 2 are gone.
    seen.borrow_mut().clear();
    fire(&handler, "ev", &seen, 1);
    assert_eq!(ids(&seen), [3]);
}

#[test]
fn nested_pass_with_only_global_callbacks_keeps_the_current_callback() {
    // A nested signal that has no callbacks of its own runs only the
    // global ones. The outer signal callback is current again when it
    // returns: remove_current then removes it.
    let handler = handler_with("void ev()");
    assert_eq!(handler.add(b"void inner()"), AddResult::Added);
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(NESTED_THEN_REMOVE, 1), false);
    handler.connect_global(key(ON_GLOBAL, 9));

    fire(&handler, "ev", &seen, 0);
    fire(&handler, "ev", &seen, 0);
    assert_eq!(
        *seen.borrow(),
        [
            (1, String::new()),
            (9, "inner".to_owned()),
            (9, "ev".to_owned()),
            (9, "ev".to_owned()),
        ]
    );
}

#[test]
fn global_callback_in_a_nested_pass_removes_itself() {
    // Inside a global callback the running signal callback is cleared, so
    // remove_current marks the global callback, not the outer signal
    // callback that started the nested pass.
    let handler = handler_with("void ev()");
    assert_eq!(handler.add(b"void inner()"), AddResult::Added);
    let seen = RefCell::new(Vec::new());
    handler.connect(b"ev", key(NESTED, 1), false);
    handler.connect_global(key(REMOVE_SELF_GLOBAL, 7));

    fire(&handler, "ev", &seen, 0);
    fire(&handler, "ev", &seen, 0);
    assert_eq!(
        *seen.borrow(),
        [
            (1, String::new()),
            (7, "inner".to_owned()),
            (1, String::new())
        ]
    );
}

#[test]
fn global_connect_and_disconnect_match_by_field() {
    // Found by callback and data only.
    let handler = handler_with("void ev()");
    let seen = RefCell::new(Vec::new());
    handler.connect_global(key(ON_GLOBAL, 9));
    handler.connect_global(key(ON_GLOBAL, 9));
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [9]);
    handler.disconnect_global(key(ON_GLOBAL, 9));
    fire(&handler, "ev", &seen, 0);
    assert_eq!(ids(&seen), [9]);
}

#[test]
fn disconnect_from_another_thread_waits_for_the_pass() {
    // After disconnect returns, the callback is not running: OBS frees the
    // callback data right after it.
    let handler = Arc::new(handler_with("void ev()"));
    handler.connect(b"ev", key(ON_SIGNAL, 1), false);
    let running = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));

    let signaller = {
        let (handler, running, done) = (handler.clone(), running.clone(), done.clone());
        std::thread::spawn(move || {
            handler.signal(
                b"ev",
                |_| {
                    running.store(true, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(100));
                    done.store(true, Ordering::SeqCst);
                },
                |_| {},
            );
        })
    };
    while !running.load(Ordering::SeqCst) {
        std::thread::yield_now();
    }
    handler.disconnect(b"ev", key(ON_SIGNAL, 1));
    assert!(
        done.load(Ordering::SeqCst),
        "disconnect returned mid-callback"
    );
    signaller.join().expect("signal thread");
}

#[test]
fn every_thread_that_waits_for_a_pass_gets_the_lock() {
    // #1748: the lock notifies only when a thread waits. With two waiters,
    // the release wakes one, and its release wakes the other.
    let handler = Arc::new(handler_with("void ev()"));
    handler.connect(b"ev", key(ON_SIGNAL, 1), false);
    handler.connect(b"ev", key(ON_SIGNAL, 2), false);
    let running = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));

    let signaller = {
        let (handler, running, done) = (handler.clone(), running.clone(), done.clone());
        std::thread::spawn(move || {
            let mut first = true;
            handler.signal(
                b"ev",
                |_| {
                    if std::mem::take(&mut first) {
                        running.store(true, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(100));
                        done.store(true, Ordering::SeqCst);
                    }
                },
                |_| {},
            );
        })
    };
    while !running.load(Ordering::SeqCst) {
        std::thread::yield_now();
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let waiters: Vec<_> = [1, 2]
        .into_iter()
        .map(|id| {
            let (handler, done, tx) = (handler.clone(), done.clone(), tx.clone());
            std::thread::spawn(move || {
                handler.disconnect(b"ev", key(ON_SIGNAL, id));
                tx.send(done.load(Ordering::SeqCst)).expect("send");
            })
        })
        .collect();
    for _ in 0..2 {
        let after_pass = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("a waiting thread never got the lock");
        assert!(after_pass, "disconnect returned mid-callback");
    }
    for waiter in waiters {
        waiter.join().expect("disconnect thread");
    }
    signaller.join().expect("signal thread");
}

//! Tier 1 mirror of `test/cmocka/test_proc.c`.
//!
//! Null receivers, `bnum_allocs`, declaration parsing, and `blog` stay with
//! that cmocka test. The safe core stores a name and a callback address. It
//! does not parse a declaration and it does not invoke the callback, so the
//! hit count of 21, the invalid-declaration log lines, and destroy from
//! inside the callback are the parity test.
//!
//! A null name on an empty handler is missing. A null name once a proc is
//! registered is also missing here. The shim faults in `strcmp`, which the
//! C test does not call.

use obs_callback::{AddOutcome, PreparedCall, ProcHandler};

const FIRST: usize = 0x1000;
const SECOND: usize = 0x2000;
const DATA_A: usize = 0x3000;
const DATA_B: usize = 0x4000;

fn ready(handler: &ProcHandler, name: &[u8]) -> (usize, usize) {
    match handler.prepare_call(Some(name)) {
        PreparedCall::Ready { func, data } => (func, data),
        PreparedCall::Missing => panic!("missing {}", String::from_utf8_lossy(name)),
    }
}

#[test]
fn create_destroy_test() {
    let handler = ProcHandler::new();
    drop(handler);
}

#[test]
fn call_passes_params_test() {
    let handler = ProcHandler::new();
    assert_eq!(handler.add(b"f", FIRST, DATA_A), AddOutcome::Added);
    assert_eq!(ready(&handler, b"f"), (FIRST, DATA_A));

    // "void  spaced()" parses. Lookup is the name, not the decl text.
    assert_eq!(handler.add(b"spaced", FIRST, DATA_A), AddOutcome::Added);
    assert_eq!(ready(&handler, b"spaced"), (FIRST, DATA_A));
    assert!(matches!(
        handler.prepare_call(Some(b"void  spaced()")),
        PreparedCall::Missing
    ));
}

#[test]
fn unknown_name_test() {
    let handler = ProcHandler::new();
    assert!(matches!(handler.prepare_call(None), PreparedCall::Missing));
    assert!(matches!(
        handler.prepare_call(Some(b"")),
        PreparedCall::Missing
    ));
    assert!(matches!(
        handler.prepare_call(Some(b"missing")),
        PreparedCall::Missing
    ));

    assert_eq!(handler.add(b"foo", FIRST, DATA_A), AddOutcome::Added);
    assert_eq!(handler.add(b"foobar", SECOND, DATA_B), AddOutcome::Added);
    assert_eq!(handler.add(b"Foo", FIRST, DATA_A), AddOutcome::Added);

    assert_eq!(ready(&handler, b"foo"), (FIRST, DATA_A));
    assert_eq!(ready(&handler, b"foobar"), (SECOND, DATA_B));
    assert_eq!(ready(&handler, b"Foo"), (FIRST, DATA_A));
    assert!(matches!(
        handler.prepare_call(Some(b"FOO")),
        PreparedCall::Missing
    ));
    assert!(matches!(
        handler.prepare_call(Some(b"fo")),
        PreparedCall::Missing
    ));
}

#[test]
fn duplicate_name_test() {
    let handler = ProcHandler::new();
    assert_eq!(handler.add(b"f", FIRST, DATA_A), AddOutcome::Added);
    assert_eq!(handler.add(b"f", SECOND, DATA_B), AddOutcome::Exists);
    assert_eq!(ready(&handler, b"f"), (FIRST, DATA_A));
}

#[test]
fn null_proc_test() {
    // Characterized, not endorsed: a null callback is stored. Calling it
    // faults, so this test only checks that the address is kept.
    let handler = ProcHandler::new();
    assert_eq!(handler.add(b"none", 0, 0), AddOutcome::Added);
    assert_eq!(ready(&handler, b"none"), (0, 0));
}

#[test]
fn reenter_test() {
    // prepare_call drops the lock before the caller runs the callback, so
    // a later add on the same handler succeeds. The hit count of 21 is the
    // parity test, which is what actually invokes the callback.
    let handler = ProcHandler::new();
    assert_eq!(handler.add(b"outer", FIRST, DATA_A), AddOutcome::Added);
    assert_eq!(ready(&handler, b"outer"), (FIRST, DATA_A));
    assert_eq!(handler.add(b"inner", SECOND, DATA_A), AddOutcome::Added);
    assert_eq!(handler.add(b"inner", FIRST, DATA_B), AddOutcome::Exists);
    assert_eq!(ready(&handler, b"inner"), (SECOND, DATA_A));
}

/// Past 16 procs the handler looks names up through a hash index (#3214):
/// every name still resolves, a missing one stays missing, and the first
/// registration still wins.
#[test]
fn many_procs_resolve_through_the_index() {
    let handler = ProcHandler::new();
    for i in 0..50_usize {
        let name = format!("p{i}");
        assert_eq!(
            handler.add(name.as_bytes(), FIRST + i, DATA_A),
            AddOutcome::Added
        );
        for j in 0..=i {
            assert_eq!(
                ready(&handler, format!("p{j}").as_bytes()),
                (FIRST + j, DATA_A)
            );
        }
        assert_eq!(handler.prepare_call(Some(b"absent")), PreparedCall::Missing);
    }
    assert_eq!(handler.add(b"p3", SECOND, DATA_B), AddOutcome::Exists);
    assert_eq!(ready(&handler, b"p3"), (FIRST + 3, DATA_A));
}

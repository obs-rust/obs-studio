//! Tier 1: every assertion of the unchanged test_hotkey_background_state.c
//! against the safe query. The backend records lock, field reads and unlock,
//! checks the optional output at each, and swaps the fields at unlock so a
//! read after unlock is caught.
use core::cell::Cell;
use obs_hotkey::background_state::{BackgroundState, NATIVE, query};

// The declared states, in the C test's order (enum obs_hotkeys_background_state).
const ACTIVE: i32 = 5;
const STARTING: i32 = 1;
const IDLE: i32 = 3;
const WAITING: i32 = 4;
const DECLINED: i32 = 6;
const CLOSED: i32 = 7;
const FAILED: i32 = 8;
const UNAVAILABLE: i32 = 2;

struct Backend<'a> {
    lock_result: bool,
    locked: bool,
    locks: u32,
    unlocks: u32,
    reads: u32,
    output: Option<&'a Cell<u32>>,
    expected_state: i32,
    expected_flags: u32,
    state: i32,
    flags: u32,
    events: String,
}

impl BackgroundState for Backend<'_> {
    fn lock(&mut self) -> bool {
        assert!(!self.locked);
        assert_eq!(self.locks, 0);
        assert_eq!(self.reads, 0);
        // Clearing the optional output precedes even a failed lock attempt.
        if let Some(output) = self.output {
            assert_eq!(output.get(), 0);
        }
        self.locks += 1;
        self.events.push('L');
        self.locked = self.lock_result;
        // Fields become current only when the lock succeeds.
        self.state = self.expected_state;
        self.flags = self.expected_flags;
        self.lock_result
    }
    fn state(&mut self) -> i32 {
        assert!(self.locked);
        self.reads += 1;
        self.events.push('S');
        self.state
    }
    fn flags(&mut self) -> u32 {
        assert!(self.locked);
        self.reads += 1;
        self.events.push('F');
        self.flags
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        assert_eq!(self.locks, 1);
        assert_eq!(self.unlocks, 0);
        assert_eq!(self.reads, if self.output.is_some() { 2 } else { 1 });
        if let Some(output) = self.output {
            assert_eq!(output.get(), self.expected_flags);
        }
        // The returned state must be captured before releasing the lock.
        self.state = NATIVE;
        self.flags = 0;
        self.locked = false;
        self.unlocks += 1;
        self.events.push('U');
    }
}

/// The C test's `query()`: one call and every count it checks afterwards.
fn run(success: bool, with_flags: bool, state: i32, flags: u32) -> String {
    let value = Cell::new(u32::MAX);
    let output = with_flags.then_some(&value);
    let mut backend = Backend {
        lock_result: success,
        locked: false,
        locks: 0,
        unlocks: 0,
        reads: 0,
        output,
        expected_state: state,
        expected_flags: flags,
        state: 0,
        flags: 0,
        events: String::new(),
    };
    let result = query(&mut backend, output);
    assert_eq!(result, if success { state } else { NATIVE });
    assert_eq!(
        value.get(),
        match (with_flags, success) {
            (false, _) => u32::MAX,
            (true, true) => flags,
            (true, false) => 0,
        }
    );
    assert_eq!(backend.locks, 1);
    assert_eq!(backend.unlocks, u32::from(success));
    assert_eq!(
        backend.reads,
        match (success, with_flags) {
            (false, _) => 0,
            (true, true) => 2,
            (true, false) => 1,
        }
    );
    assert!(!backend.locked);
    backend.events
}

#[test]
fn failed_lock() {
    assert_eq!(run(false, true, ACTIVE, u32::MAX), "L");
    assert_eq!(run(false, false, ACTIVE, u32::MAX), "L");
}

#[test]
fn states_and_flags() {
    let states = [
        NATIVE,
        ACTIVE,
        STARTING,
        IDLE,
        WAITING,
        DECLINED,
        CLOSED,
        FAILED,
        UNAVAILABLE,
        // Characterized, not endorsed: unknown native enum values pass through.
        12345,
    ];
    let flags = [0, 1, 0x8000_0000, 0xa5a5_1234, u32::MAX];
    for state in states {
        for flag in flags {
            assert_eq!(run(true, true, state, flag), "LSFU");
            assert_eq!(run(true, false, state, flag), "LSU");
        }
    }
}

#[test]
fn edge_any_state_and_flags_pass_through() {
    // Beyond the C test: negative and extreme raw values pass through as well.
    for state in [i32::MIN, -1, 9, i32::MAX] {
        for flag in [0, 0x3f, u32::MAX] {
            for with_flags in [false, true] {
                run(true, with_flags, state, flag);
                run(false, with_flags, state, flag);
            }
        }
    }
}

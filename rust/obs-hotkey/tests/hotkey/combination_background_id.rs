//! Tier 1: every assertion of the unchanged
//! test_hotkey_combination_background_id.c against the safe query. The
//! backend records lock, field read, unlock, call and free, and swaps the
//! callback slot at unlock so a read after unlock is caught.
use obs_hotkey::combination_background_id::{CombinationBackgroundId, query};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Output {
    None,
    Empty,
    Text,
    Partial,
    FailedEmpty,
    Reenter,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Empty,
    Callback,
    Wrong,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Combo {
    modifiers: u32,
    key: i32,
}

struct Backend {
    lock_result: bool,
    locked: bool,
    with_callback: bool,
    slot: Slot,
    combo: Combo,
    output: Output,
    events: String,
    locks: u32,
    unlocks: u32,
    reads: u32,
    calls: u32,
    frees: u32,
    allocation: Option<*const u8>,
}

impl CombinationBackgroundId for Backend {
    type Callback = Slot;
    type Id = Option<String>;
    type Owned = Option<String>;
    fn lock(&mut self) -> bool {
        assert!(!self.locked);
        self.locks += 1;
        self.events.push('L');
        self.locked = self.lock_result;
        // The current callback becomes available only when locking succeeds.
        self.slot = if self.with_callback {
            Slot::Callback
        } else {
            Slot::Empty
        };
        self.lock_result
    }
    fn callback(&mut self) -> Option<Slot> {
        assert!(self.locked);
        self.reads += 1;
        self.events.push('R');
        (self.slot != Slot::Empty).then_some(self.slot)
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        assert_eq!(self.reads, self.locks);
        self.unlocks += 1;
        self.events.push('U');
        self.locked = false;
        // The wrapper must invoke its saved callback, not read this after unlock.
        self.slot = Slot::Wrong;
    }
    fn empty(&mut self) -> Option<String> {
        None
    }
    fn invoke(&mut self, callback: Slot, id: &mut Option<String>) -> bool {
        assert_eq!(callback, Slot::Callback, "wrong callback invoked");
        assert!(!self.locked);
        assert_eq!(self.unlocks, 1);
        assert_eq!(self.calls, 0);
        self.calls += 1;
        self.events.push('C');
        assert!(id.is_none());
        match self.output {
            Output::Reenter => {
                // Reenter the query after release, with no nested callback.
                self.with_callback = false;
                assert!(query(self).is_none());
            }
            Output::None | Output::FailedEmpty => {}
            output => {
                let text = if output == Output::Empty {
                    String::with_capacity(1)
                } else {
                    "background-id".to_owned()
                };
                self.allocation = Some(text.as_ptr());
                *id = Some(text);
            }
        }
        !matches!(self.output, Output::Partial | Output::FailedEmpty)
    }
    fn free(&mut self, id: Option<String>) {
        assert!(!self.locked);
        self.frees += 1;
        self.events.push('F');
        assert_eq!(id.as_ref().map(|id| id.as_ptr()), self.allocation);
    }
    fn transfer(&mut self, id: Option<String>) -> Option<String> {
        id
    }
}

fn run(success: bool, with_callback: bool, output: Output, combo: Combo) {
    let mut backend = Backend {
        lock_result: success,
        locked: false,
        with_callback,
        slot: Slot::Wrong,
        combo,
        output,
        events: String::new(),
        locks: 0,
        unlocks: 0,
        reads: 0,
        calls: 0,
        frees: 0,
        allocation: None,
    };
    let result = query(&mut backend);
    let invoked = success && with_callback;
    let nested = invoked && output == Output::Reenter;
    let b = &backend;
    assert!(!b.locked);
    assert_eq!(b.combo, combo);
    assert_eq!(b.locks, if nested { 2 } else { 1 });
    assert_eq!(
        b.unlocks,
        match (success, nested) {
            (false, _) => 0,
            (true, true) => 2,
            (true, false) => 1,
        }
    );
    assert_eq!(b.reads, b.unlocks);
    assert_eq!(b.calls, u32::from(invoked));
    let freed = success
        && (!with_callback || matches!(output, Output::Partial | Output::FailedEmpty) || nested);
    assert_eq!(b.frees, u32::from(freed));
    assert_eq!(
        b.events,
        expected_events(success, with_callback, output),
        "{success} {with_callback} {output:?}"
    );
    check_result(result, invoked, output, b.allocation);
}

fn expected_events(success: bool, with_callback: bool, output: Output) -> &'static str {
    if !success {
        "L"
    } else if !with_callback {
        "LRUF"
    } else if output == Output::Reenter {
        "LRUCLRUF"
    } else if matches!(output, Output::Partial | Output::FailedEmpty) {
        "LRUCF"
    } else {
        "LRUC"
    }
}

fn check_result(
    result: Option<Option<String>>,
    invoked: bool,
    output: Output,
    allocation: Option<*const u8>,
) {
    if invoked && matches!(output, Output::Empty | Output::Text) {
        // Exact ownership transfer: the callback's allocation, not a copy.
        let id = result.flatten().expect("successful identifier");
        assert_eq!(Some(id.as_ptr()), allocation);
        assert_eq!(
            id,
            if output == Output::Empty {
                ""
            } else {
                "background-id"
            }
        );
    } else if invoked && matches!(output, Output::None | Output::Reenter) {
        // Characterized, not endorsed: callback success may return NULL.
        assert_eq!(result, Some(None));
    } else {
        assert_eq!(result, None);
    }
}

#[test]
fn lock_and_callback_contract() {
    let combinations = [
        Combo {
            modifiers: 0,
            key: 0,
        },
        // INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY with OBS_KEY_A.
        Combo {
            modifiers: (1 << 2) | (1 << 1),
            key: 1,
        },
        Combo {
            modifiers: u32::MAX,
            key: -1,
        },
    ];
    for combo in combinations {
        run(false, false, Output::None, combo);
        run(false, true, Output::Partial, combo);
        run(true, false, Output::None, combo);
        run(true, true, Output::None, combo);
        run(true, true, Output::Empty, combo);
        run(true, true, Output::Text, combo);
        run(true, true, Output::Partial, combo);
        run(true, true, Output::FailedEmpty, combo);
        run(true, true, Output::Reenter, combo);
    }
}

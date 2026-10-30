//! Tier 2 layout and Tier 3 parity: the shims against the original
//! `libobs/obs-hotkey-binding-press.c.h` (oracle/hotkey_binding_press.c).
//!
//! The unchanged C test `test_hotkey_binding_press_steps` compiles the C into
//! itself, so it characterizes the C only; `test_hotkey_press_state` sees the
//! Rust swap through a real libobs. This file compares the two
//! implementations directly, on every OS, over every state and over more
//! sequences than the C test scripts.
//!
//! Both sides run on the same fake bindings. This file defines the two hidden
//! seams the C and the shims call (`obs_hotkey_binding_count_fields` and
//! `obs_hotkey_dispatch`) and logs every call, per thread: each accessor call,
//! with the binding; and each dispatch, with the hotkey, the pressed value and
//! the flag and count the dispatch sees. So an accessor call made at the wrong
//! point, a write through a copy instead of the binding, a write after the
//! dispatch, or a dispatch chosen on the stale count changes the log or the
//! fake state.
//!
//! A run feeds one sequence of events and of state changes from outside to a
//! C world and a Rust world, and compares after every step: the call log one
//! to one (order and count) and the flag of every binding and the count of
//! every hotkey.
//!
//! Intentional differences, excluded here: a null binding, or a null field
//! pointer from the accessor, does nothing in Rust; C dereferences it
//! (`null_binding_does_nothing` checks the Rust side). Nothing else is
//! excluded.

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};

use crate::hotkey::binding_press_hooks::{self, Hooks};
use obs_c_oracle::hotkey_binding_press::{
    LAYOUT_NAMES, oracle_hotkey_binding_press_layout, oracle_obs_hotkey_binding_press,
    oracle_obs_hotkey_binding_release,
};
use obs_hotkey::ffi::binding_press::{
    obs_hotkey_binding, obs_hotkey_binding_count_fields as Fields,
    obs_hotkey_binding_press as rust_press, obs_hotkey_binding_release as rust_release,
    obs_hotkey_t,
};
use proptest::prelude::*;

#[test]
fn layout_matches_c() {
    let rust = [
        size_of::<Fields>(),
        align_of::<Fields>(),
        offset_of!(Fields, pressed),
        offset_of!(Fields, count),
        offset_of!(Fields, hotkey),
        size_of::<bool>(),
        size_of::<i32>(),
    ];
    for (i, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_hotkey_binding_press_layout(i) };
        assert_eq!(value, c, "{name}");
    }
}

// ---------------------------------------------------------------------------
// The fake libobs side

/// One hotkey: the count the unit writes.
#[repr(C)]
#[derive(Debug)]
struct FakeHotkey {
    pressed: i32,
    /// Its index in the world, for the log.
    index: usize,
}

/// One binding: its flag and the hotkey whose count it shares with the
/// hotkey's other bindings.
#[repr(C)]
#[derive(Debug)]
struct FakeBinding {
    pressed: bool,
    hotkey: *mut FakeHotkey,
    /// Its index in the world, for the log.
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// One call into the seams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Fields(usize),
    /// A dispatch: the hotkey, the pressed value, and the flag and count it
    /// sees.
    Dispatch {
        hotkey: usize,
        pressed: bool,
        flag: bool,
        count: i32,
    },
}

#[derive(Default)]
struct Seams {
    side: Option<Side>,
    bindings: Vec<*mut FakeBinding>,
    log: Vec<Call>,
    /// Which accessor field to return as null (Rust side only).
    null_field: Option<usize>,
    /// The binding of the accessor call being serviced, for the dispatch to
    /// read its flag (a hotkey may have several bindings).
    current: Option<usize>,
}

thread_local! {
    static SEAMS: RefCell<Seams> = RefCell::new(Seams::default());
}

fn with_seams<R>(f: impl FnOnce(&mut Seams) -> R) -> R {
    SEAMS.with(|s| f(&mut s.borrow_mut()))
}

/// The binding accessor (`obs-hotkey.c.h`), installed in `binding_press_hooks`
/// for this world.
///
/// # Safety
///
/// `binding` points at a live [`FakeBinding`].
unsafe fn accessor(binding: *mut obs_hotkey_binding) -> Fields {
    let b = binding.cast::<FakeBinding>();
    // SAFETY: a live FakeBinding, per the caller; its hotkey is too.
    let (index, hotkey) = unsafe { ((*b).index, (*b).hotkey) };
    let null_field = with_seams(|s| {
        s.log.push(Call::Fields(index));
        s.current = Some(index);
        s.null_field
    });
    // SAFETY: as above; the pointers are to the binding's own flag and its
    // hotkey's own count.
    let mut fields = unsafe {
        Fields {
            pressed: &raw mut (*b).pressed,
            count: &raw mut (*hotkey).pressed,
            hotkey: hotkey.cast::<obs_hotkey_t>(),
        }
    };
    match null_field {
        Some(0) => fields.pressed = std::ptr::null_mut(),
        Some(1) => fields.count = std::ptr::null_mut(),
        Some(_) => fields.hotkey = std::ptr::null_mut(),
        None => {}
    }
    fields
}

/// The dispatch (`obs-hotkey.c.h`): it records the hotkey, the pressed value
/// and the flag and count it sees, and makes no other choice.
///
/// # Safety
///
/// `hotkey` points at a live [`FakeHotkey`].
unsafe fn dispatch(hotkey: *mut obs_hotkey_t, pressed: bool) {
    let h = hotkey.cast::<FakeHotkey>();
    // SAFETY: a live FakeHotkey, per the caller.
    let (index, count) = unsafe { ((*h).index, (*h).pressed) };
    // The flag is the accessor call's binding's: the dispatch follows the
    // accessor in both implementations.
    let flag = with_seams(|s| {
        s.current
            .and_then(|i| s.bindings.get(i))
            .map(|&b| {
                // SAFETY: a live binding of this world.
                unsafe { (*b).pressed }
            })
            .unwrap_or(false)
    });
    with_seams(|s| {
        s.log.push(Call::Dispatch {
            hotkey: index,
            pressed,
            flag,
            count,
        })
    });
}

/// Runs one event of one binding of this world's side.
fn fire(binding: usize, pressed: bool) {
    let (side, b) = with_seams(|s| (s.side.expect("a side"), s.bindings[binding]));
    let b = b.cast::<obs_hotkey_binding>();
    // SAFETY: a live binding, per the world; the caller holds the hotkey
    // mutex in libobs, which this single-threaded world stands in for.
    unsafe {
        match (side, pressed) {
            (Side::C, true) => oracle_obs_hotkey_binding_press(b.cast::<c_void>()),
            (Side::C, false) => oracle_obs_hotkey_binding_release(b.cast::<c_void>()),
            (Side::Rust, true) => rust_press(b),
            (Side::Rust, false) => rust_release(b),
        }
    }
}

// ---------------------------------------------------------------------------
// Runs

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// An event of one binding.
    Event { binding: usize, pressed: bool },
    /// A state change from outside: the binding's flag and the count of the
    /// hotkey it belongs to (all of the hotkey's bindings share it).
    Set {
        binding: usize,
        flag: bool,
        count: i32,
    },
}

/// One binding's start state: its flag, and the count of the hotkey it
/// belongs to (`hotkey` names it, so two bindings can share one count).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Start {
    flags: bool,
    count: i32,
    hotkey: usize,
}

/// What one step left: the call log, and every binding's flag and every
/// hotkey's count.
type Trace = Vec<(Vec<Call>, Vec<(bool, i32)>)>;

fn run(side: Side, start: &[Start], steps: &[Step]) -> Trace {
    // One `FakeHotkey` per distinct hotkey; a binding names its hotkey's
    // index in `start`, so two bindings of one hotkey share the count.
    let num_hotkeys = start.iter().map(|s| s.hotkey + 1).max().unwrap_or(0);
    let mut hotkeys: Vec<Box<FakeHotkey>> = (0..num_hotkeys)
        .map(|index| {
            let count = start
                .iter()
                .find(|s| s.hotkey == index)
                .map(|s| s.count)
                .unwrap_or(0);
            Box::new(FakeHotkey {
                pressed: count,
                index,
            })
        })
        .collect();
    let mut bindings: Vec<Box<FakeBinding>> = start
        .iter()
        .enumerate()
        .map(|(index, s)| {
            Box::new(FakeBinding {
                pressed: s.flags,
                hotkey: &raw mut *hotkeys[s.hotkey],
                index,
            })
        })
        .collect();
    let ptrs: Vec<*mut FakeBinding> = bindings.iter_mut().map(|b| &raw mut **b).collect();
    with_seams(|s| {
        *s = Seams {
            side: Some(side),
            bindings: ptrs.clone(),
            ..Seams::default()
        }
    });
    // The hooks cast to this world's own types; it is alive until they are
    // cleared below.
    binding_press_hooks::install(Hooks {
        // SAFETY: the pointers come from this world.
        accessor: |b| unsafe { accessor(b) },
        // SAFETY: as above.
        dispatch: |h, pressed| unsafe { dispatch(h, pressed) },
    });
    let mut trace = Vec::new();
    for step in steps {
        match step {
            Step::Event { binding, pressed } => fire(*binding, *pressed),
            Step::Set {
                binding,
                flag,
                count,
            } => {
                let b = ptrs[*binding];
                // SAFETY: a live binding of this world.
                unsafe {
                    (*b).pressed = *flag;
                    (*(*b).hotkey).pressed = *count;
                }
            }
        }
        let log = with_seams(|s| std::mem::take(&mut s.log));
        // SAFETY: the bindings are alive until the end of this function.
        let state = ptrs
            .iter()
            .map(|&b| unsafe { ((*b).pressed, (*(*b).hotkey).pressed) })
            .collect();
        trace.push((log, state));
    }
    binding_press_hooks::clear();
    with_seams(|s| *s = Seams::default());
    drop(bindings);
    drop(hotkeys);
    trace
}

/// Runs both sides and compares them after every step.
fn check(start: &[Start], steps: &[Step]) -> Result<(), String> {
    let c = run(Side::C, start, steps);
    let rust = run(Side::Rust, start, steps);
    for (i, (c, rust)) in c.iter().zip(&rust).enumerate() {
        if c != rust {
            return Err(format!(
                "step {i} of {steps:?} from {start:?}:\n  C:    {c:?}\n  Rust: {rust:?}"
            ));
        }
    }
    Ok(())
}

fn event(binding: usize, pressed: bool) -> Step {
    Step::Event { binding, pressed }
}

const COUNTS: [i32; 6] = [-2, -1, 0, 1, 2, 3];

/// Every state, both events: the C test's table, one binding at a time, in a
/// sequence of two steps so a state set from outside is compared as well.
#[test]
fn every_state_matches_c() {
    for flag in [false, true] {
        for count in COUNTS {
            for pressed in [false, true] {
                check(
                    &[Start {
                        flags: false,
                        count: 0,
                        hotkey: 0,
                    }],
                    &[
                        Step::Set {
                            binding: 0,
                            flag,
                            count,
                        },
                        event(0, pressed),
                    ],
                )
                .unwrap();
            }
        }
    }
}

/// Two bindings of one hotkey: the count is shared, so the first press fires,
/// the second does not, and only the last release does. Every sequence of
/// four events from every start latch state.
#[test]
fn two_bindings_share_the_count() {
    // Both bindings of the hotkey see the same count; the start state gives
    // them the same count and different flags.
    let start = [
        Start {
            flags: false,
            count: 0,
            hotkey: 0,
        },
        Start {
            flags: false,
            count: 0,
            hotkey: 0,
        },
    ];
    for a in [false, true] {
        for b in [false, true] {
            for c in [false, true] {
                for d in [false, true] {
                    check(
                        &start,
                        &[event(0, a), event(1, b), event(0, c), event(1, d)],
                    )
                    .unwrap();
                }
            }
        }
    }
}

/// Sequences over two bindings from a pressed state, which is where the count
/// and the flags disagree.
#[test]
fn sequences_from_a_pressed_state() {
    let start = [
        Start {
            flags: true,
            count: 2,
            hotkey: 0,
        },
        Start {
            flags: false,
            count: 2,
            hotkey: 0,
        },
    ];
    for a in [false, true] {
        for b in [false, true] {
            for c in [false, true] {
                check(&start, &[event(0, a), event(1, b), event(0, c)]).unwrap();
            }
        }
    }
}

/// A null binding, or a field pointer the accessor returned as null, does
/// nothing in Rust (the intentional difference; C dereferences them).
#[test]
fn null_binding_does_nothing() {
    // SAFETY: the shims only check the pointer before using it.
    unsafe {
        rust_press(std::ptr::null_mut());
        rust_release(std::ptr::null_mut());
    }
    let mut hotkey = FakeHotkey {
        pressed: 0,
        index: 0,
    };
    let mut binding = FakeBinding {
        pressed: false,
        hotkey: &raw mut hotkey,
        index: 0,
    };
    let ptr: *mut FakeBinding = &raw mut binding;
    let b = ptr.cast::<obs_hotkey_binding>();
    for null_field in 0..3 {
        with_seams(|s| {
            *s = Seams {
                side: Some(Side::Rust),
                bindings: vec![ptr],
                null_field: Some(null_field),
                ..Seams::default()
            }
        });
        binding_press_hooks::install(Hooks {
            // SAFETY: the pointer comes from this world.
            accessor: |b| unsafe { accessor(b) },
            // SAFETY: as above.
            dispatch: |h, pressed| unsafe { dispatch(h, pressed) },
        });
        // SAFETY: a live binding, per the world.
        unsafe {
            rust_press(b);
            rust_release(b);
        }
        let log = with_seams(|s| std::mem::take(&mut s.log));
        // One accessor call per event, and the dispatch is skipped.
        assert_eq!(
            log,
            vec![Call::Fields(0), Call::Fields(0)],
            "null field {null_field}"
        );
    }
    // SAFETY: not dereferenced by the shim.
    assert!(!unsafe { (*ptr).pressed });
    // SAFETY: as above.
    assert_eq!(hotkey.pressed, 0);
    binding_press_hooks::clear();
    with_seams(|s| *s = Seams::default());
}

// ---------------------------------------------------------------------------
// Random runs

proptest! {
    /// Up to three bindings, random start states and up to 40 steps, half of
    /// them state changes from outside, with counts kept in the C's range so
    /// the two agree exactly.
    #[test]
    fn random_runs_match_c(
        start in proptest::collection::vec((any::<bool>(), -3i32..4), 1..4),
        steps in proptest::collection::vec(
            prop_oneof![
                (0usize..3, any::<bool>()).prop_map(|(binding, pressed)| event(binding, pressed)),
                (0usize..3, any::<bool>(), -3i32..4)
                    .prop_map(|(binding, flag, count)| Step::Set { binding, flag, count }),
            ],
            0..40,
        ),
    ) {
        let bindings = start.len();
        let start: Vec<Start> = start
            .into_iter()
            .enumerate()
            .map(|(index, (flags, count))| Start {
                flags,
                count,
                // Every other binding of the world can share a hotkey with an
                // earlier one, which is how libobs holds two bindings.
                hotkey: if index > 0 && index % 2 == 0 { index - 1 } else { index },
            })
            .collect();
        // The generator makes up to three bindings; keep the events in range.
        let steps: Vec<Step> = steps
            .into_iter()
            .filter_map(|step| match step {
                Step::Event { binding, pressed } if binding < bindings => {
                    Some(event(binding, pressed))
                }
                Step::Set { binding, flag, count } if binding < bindings => {
                    Some(Step::Set { binding, flag, count })
                }
                _ => None,
            })
            .collect();
        check(&start, &steps).unwrap();
    }
}

//! Tier 1 mirrors of test_source_show_active_counters.c. The C
//! `obs_source_activate()` / `obs_source_deactivate()` the shims forward to
//! are modelled here on `[show_refs, activate_refs]`, as libobs does it for a
//! source with no active children.
use obs_source::show_active_counters::{AUX_VIEW, Change, Direction, MAIN_VIEW, apply};
use std::cell::RefCell;

/// The C calls a change made, and the names it logged.
#[derive(Default)]
struct Calls {
    forwarded: RefCell<Vec<(Direction, u32)>>,
    logs: RefCell<Vec<&'static str>>,
}

/// Model of `obs_source_activate()` / `obs_source_deactivate()` on
/// `[show_refs, activate_refs]`.
fn model(refs: &mut [i64; 2], direction: Direction, view_type: u32) {
    let [show, activate] = refs;
    match direction {
        Direction::Activate => {
            *show += 1;
            if view_type == MAIN_VIEW {
                *activate += 1;
            }
        }
        Direction::Deactivate => {
            if *show > 0 {
                *show -= 1;
            }
            if view_type == MAIN_VIEW && *activate > 0 {
                *activate -= 1;
            }
        }
    }
}

fn call(change: Change, refs: Option<&mut [i64; 2]>, calls: &Calls) {
    apply(
        change,
        refs,
        |refs, direction, view_type| {
            calls.forwarded.borrow_mut().push((direction, view_type));
            model(refs, direction, view_type);
        },
        |name| calls.logs.borrow_mut().push(name.to_str().unwrap()),
    );
}

/// null_source_changes_nothing_and_names_the_caller: NULL forwards nothing
/// and logs once under the name of the function that was called.
#[test]
fn null_source_changes_nothing_and_names_the_caller() {
    let calls = Calls::default();
    for change in Change::ALL {
        call(change, None, &calls);
    }
    assert!(calls.forwarded.borrow().is_empty());
    assert_eq!(
        *calls.logs.borrow(),
        [
            "obs_source_inc_showing",
            "obs_source_inc_active",
            "obs_source_dec_showing",
            "obs_source_dec_active",
        ]
    );
}

/// Each change forwards to one C call with its own view type.
#[test]
fn each_forwards_its_call_and_view_type() {
    let expected = [
        (Change::IncShowing, Direction::Activate, AUX_VIEW),
        (Change::IncActive, Direction::Activate, MAIN_VIEW),
        (Change::DecShowing, Direction::Deactivate, AUX_VIEW),
        (Change::DecActive, Direction::Deactivate, MAIN_VIEW),
    ];
    for (change, direction, view_type) in expected {
        assert_eq!(change.direction(), direction);
        assert_eq!(change.view_type(), view_type);
        let calls = Calls::default();
        call(change, Some(&mut [0, 0]), &calls);
        assert_eq!(*calls.forwarded.borrow(), [(direction, view_type)]);
        assert!(calls.logs.borrow().is_empty());
    }
    assert_eq!((MAIN_VIEW, AUX_VIEW), (1, 2));
}

/// each_moves_only_its_counters: inc_showing raises show_refs, inc_active
/// raises both, dec_showing lowers show_refs and dec_active both, each
/// decrement only above zero, over every pair of edge values.
#[test]
fn each_moves_only_its_counters() {
    let edges = [0, 1, 2, -1, -2, 0x5a5a];
    let calls = Calls::default();
    for change in Change::ALL {
        let (show_step, activate_step) = match change {
            Change::IncShowing => (1, 0),
            Change::IncActive => (1, 1),
            Change::DecShowing => (-1, 0),
            Change::DecActive => (-1, -1),
        };
        let step = |value: i64, step: i64| {
            if step < 0 && value <= 0 {
                value
            } else {
                value + step
            }
        };
        for show in edges {
            for activate in edges {
                let mut refs = [show, activate];
                call(change, Some(&mut refs), &calls);
                assert_eq!(
                    refs,
                    [step(show, show_step), step(activate, activate_step)],
                    "{change:?} from {show}/{activate}"
                );
            }
        }
    }
    assert!(calls.logs.borrow().is_empty());
}

/// balanced_sequence_ends_at_zero.
#[test]
fn balanced_sequence_ends_at_zero() {
    let calls = Calls::default();
    let mut refs = [0, 0];
    for change in [Change::IncActive, Change::IncShowing, Change::IncActive] {
        call(change, Some(&mut refs), &calls);
    }
    assert_eq!(refs, [3, 2]);
    call(Change::DecShowing, Some(&mut refs), &calls);
    assert_eq!(refs, [2, 2]);
    call(Change::DecActive, Some(&mut refs), &calls);
    call(Change::DecActive, Some(&mut refs), &calls);
    assert_eq!(refs, [0, 0]);
    call(Change::DecActive, Some(&mut refs), &calls);
    call(Change::DecShowing, Some(&mut refs), &calls);
    assert_eq!(refs, [0, 0]);
    call(Change::IncShowing, Some(&mut refs), &calls);
    call(Change::DecActive, Some(&mut refs), &calls);
    assert_eq!(refs, [0, 0]);
    assert!(calls.logs.borrow().is_empty());
}

/// The names match the C functions exactly.
#[test]
fn function_names() {
    let names: Vec<_> = Change::ALL.map(Change::function).into();
    assert_eq!(
        names,
        [
            c"obs_source_inc_showing",
            c"obs_source_inc_active",
            c"obs_source_dec_showing",
            c"obs_source_dec_active",
        ]
    );
}

//! Tier 1: `obs_output::encoder_group_num_started`, the safe core of
//! `obs_encoder_group_num_started()`.
//!
//! The `characterization_*` cases port every assertion of the unchanged
//! `test/cmocka/test_encoder_group_num_started.c` onto two recording groups,
//! its two cases one for one: each query locks the selected group's mutex
//! exactly once and unlocks it exactly once, never nested and never the other
//! group's; the exact count comes back over the full `u32` range (0, 1, 2,
//! 32, `u32::MAX - 1`, `u32::MAX`) for the queried group and its complement
//! for the other, with unrelated lifecycle state set and nothing in either
//! group changed; and a count rewritten by the lock and again by the unlock
//! returns the value seen under the lock and leaves the unlock's value.
//!
//! The `edge_*` cases are new: the one read happens between the one lock
//! and the one unlock, in that order, and a proptest over any count and any
//! rewrite in the lock or the unlock against the C's snapshot rule.

use obs_output::encoder_group_num_started::{StartedCount, num_started};
use proptest::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Lock(usize),
    Read(usize),
    Unlock(usize),
}

/// `struct obs_encoder_group` as the C test sets it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Group {
    num_encoders_started: u32,
    destroy_on_stop: bool,
    frame_rate_divisors_lcm: u32,
}

/// The C test's statics: both groups, the selected one, the call counts,
/// the lock state and the rewrites the lock and unlock make.
#[derive(Debug, Default)]
struct Fixture {
    groups: [Group; 2],
    selected: Option<usize>,
    lock_calls: u32,
    unlock_calls: u32,
    locked: bool,
    change_on_lock: bool,
    change_on_unlock: bool,
    locked_value: u32,
    unlocked_value: u32,
    events: Vec<Event>,
}

/// The `struct obs_encoder_group *` the query was given.
struct Query<'a> {
    fixture: &'a mut Fixture,
    group: usize,
}

impl StartedCount for Query<'_> {
    /// The test's `pthread_mutex_lock()`.
    fn lock(&mut self) {
        let f = &mut *self.fixture;
        assert_eq!(Some(self.group), f.selected, "the selected group's mutex");
        assert!(!f.locked);
        assert_eq!(f.lock_calls, 0);
        assert_eq!(f.unlock_calls, 0);
        f.lock_calls += 1;
        f.locked = true;
        if f.change_on_lock {
            f.groups[self.group].num_encoders_started = f.locked_value;
        }
        f.events.push(Event::Lock(self.group));
    }
    fn read(&mut self) -> u32 {
        let f = &mut *self.fixture;
        f.events.push(Event::Read(self.group));
        f.groups[self.group].num_encoders_started
    }
    /// The test's `pthread_mutex_unlock()`.
    fn unlock(&mut self) {
        let f = &mut *self.fixture;
        assert_eq!(Some(self.group), f.selected, "the selected group's mutex");
        assert!(f.locked);
        assert_eq!(f.lock_calls, 1);
        assert_eq!(f.unlock_calls, 0);
        f.unlock_calls += 1;
        if f.change_on_unlock {
            f.groups[self.group].num_encoders_started = f.unlocked_value;
        }
        f.locked = false;
        f.events.push(Event::Unlock(self.group));
    }
}

/// `query()`.
fn query(fixture: &mut Fixture, group: usize) -> u32 {
    fixture.selected = Some(group);
    fixture.lock_calls = 0;
    fixture.unlock_calls = 0;
    let result = num_started(&mut Query { fixture, group });
    assert_eq!(fixture.lock_calls, 1);
    assert_eq!(fixture.unlock_calls, 1);
    assert!(!fixture.locked);
    result
}

/// `setup()`.
fn setup() -> Fixture {
    Fixture::default()
}

#[test]
fn characterization_exact_count_and_group() {
    let mut f = setup();
    for value in [0, 1, 2, 32, u32::MAX - 1, u32::MAX] {
        f.groups[0].num_encoders_started = value;
        f.groups[1].num_encoders_started = !value;
        // Unrelated lifecycle state must not clamp or gate the count.
        f.groups[0].destroy_on_stop = true;
        f.groups[1].frame_rate_divisors_lcm = u32::MAX;
        let before = f.groups;
        assert_eq!(query(&mut f, 0), value);
        assert_eq!(query(&mut f, 1), !value);
        assert_eq!(f.groups, before);
    }
}

#[test]
fn characterization_snapshot_inside_mutex_calls() {
    let mut f = setup();
    f.change_on_lock = true;
    f.change_on_unlock = true;
    for value in [0, 1, u32::MAX] {
        f.locked_value = value;
        f.unlocked_value = !value;
        f.groups[0].num_encoders_started = f.unlocked_value;
        assert_eq!(query(&mut f, 0), value);
        assert_eq!(f.groups[0].num_encoders_started, f.unlocked_value);
    }
}

#[test]
fn edge_one_read_between_the_lock_and_the_unlock() {
    for group in [0, 1] {
        let mut f = setup();
        f.groups[group].num_encoders_started = 7;
        assert_eq!(query(&mut f, group), 7);
        assert_eq!(
            f.events,
            [Event::Lock(group), Event::Read(group), Event::Unlock(group)]
        );
    }
}

proptest! {
    #[test]
    fn edge_any_count_returns_the_value_under_the_lock(
        start in any::<u32>(),
        other in any::<u32>(),
        on_lock in prop::option::of(any::<u32>()),
        on_unlock in prop::option::of(any::<u32>()),
        group in 0..2usize,
    ) {
        let mut f = setup();
        f.groups[group].num_encoders_started = start;
        f.groups[1 - group].num_encoders_started = other;
        f.change_on_lock = on_lock.is_some();
        f.locked_value = on_lock.unwrap_or(0);
        f.change_on_unlock = on_unlock.is_some();
        f.unlocked_value = on_unlock.unwrap_or(0);
        let result = query(&mut f, group);
        prop_assert_eq!(result, on_lock.unwrap_or(start));
        prop_assert_eq!(
            f.groups[group].num_encoders_started,
            on_unlock.or(on_lock).unwrap_or(start)
        );
        prop_assert_eq!(f.groups[1 - group].num_encoders_started, other);
        prop_assert_eq!(
            &f.events,
            &[Event::Lock(group), Event::Read(group), Event::Unlock(group)]
        );
    }
}

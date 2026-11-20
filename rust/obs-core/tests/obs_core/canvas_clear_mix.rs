//! Tier 1: `obs_core::canvas_clear_mix`, the safe core of
//! `obs_canvas_clear_mix()`.
//!
//! The `characterization_*` cases port every assertion of the unchanged
//! `test/cmocka/test_canvas_clear_mix.c` onto a recording mix list: no mix
//! (no lock, no list read, nothing freed, a held mutex left alone), one
//! mix, each position of four, an unregistered mix (detached, not freed)
//! and a duplicated pointer (only the first entry erased and freed once);
//! the free runs under the lock after the erase while the canvas still
//! holds the mix, the canvas is cleared only after the unlock, and a second
//! clear neither locks nor frees. The C fixture's byte comparisons of the
//! canvas, its references and its source are the untouched recorder state
//! here; Tier 3 compares the real structures through the shim.
//!
//! The `edge_*` cases are new: the exact read trace of the C loop (the
//! length read before each step, each entry read before the canvas's mix is
//! read again for its comparison) and a proptest over any list, duplicates
//! and absent mixes included, against a model of the C.

use obs_core::canvas_clear_mix::{Mixes, clear_mix};
use proptest::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    CanvasRead,
    Detach,
    Lock,
    Unlock,
    Count,
    At(usize),
    Erase(usize),
    Free(Option<u32>),
}

/// A canvas and the core's mix list, mixes named by number.
#[derive(Debug, Default)]
struct Core {
    canvas: Option<u32>,
    list: Vec<Option<u32>>,
    locked: bool,
    /// The mutex is already held by someone else: a lock would deadlock.
    held: bool,
    events: Vec<Event>,
    freed: Vec<Option<u32>>,
}

impl Mixes for Core {
    type Mix = u32;
    fn canvas_mix(&mut self) -> Option<u32> {
        self.events.push(Event::CanvasRead);
        self.canvas
    }
    fn detach(&mut self) {
        assert!(!self.locked, "the canvas is cleared only after the unlock");
        self.events.push(Event::Detach);
        self.canvas = None;
    }
    fn lock(&mut self) {
        assert!(!self.locked && !self.held, "unexpected recursive lock");
        assert!(self.canvas.is_some());
        self.events.push(Event::Lock);
        self.locked = true;
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        assert!(self.canvas.is_some(), "the mix is still attached at unlock");
        self.events.push(Event::Unlock);
        self.locked = false;
    }
    fn count(&mut self) -> usize {
        assert!(self.locked);
        self.events.push(Event::Count);
        self.list.len()
    }
    fn at(&mut self, index: usize) -> Option<u32> {
        assert!(self.locked);
        self.events.push(Event::At(index));
        self.list[index]
    }
    fn erase(&mut self, index: usize) {
        assert!(self.locked);
        self.events.push(Event::Erase(index));
        self.list.remove(index);
    }
    fn free(&mut self, mix: Option<u32>) {
        assert!(self.locked, "freed under the lock");
        assert_eq!(self.canvas, mix, "freed while the canvas still holds it");
        self.events.push(Event::Free(mix));
        self.freed.push(mix);
    }
}

fn count_of(core: &Core, event: Event) -> usize {
    core.events.iter().filter(|e| **e == event).count()
}

/// `list_case()`: `count` registered mixes, the canvas holding the one at
/// `position`.
fn list_case(count: u32, position: u32) {
    let mut core = Core {
        list: (0..count).map(Some).collect(),
        canvas: Some(position),
        ..Core::default()
    };
    clear_mix(&mut core);
    assert_eq!(core.canvas, None);
    assert_eq!(core.freed, [Some(position)]);
    assert_eq!(count_of(&core, Event::Lock), 1);
    assert_eq!(count_of(&core, Event::Unlock), 1);
    let rest: Vec<_> = (0..count).filter(|&i| i != position).map(Some).collect();
    assert_eq!(core.list, rest);
    // Erased before it is freed, so the list is one shorter at the free.
    let erase = core
        .events
        .iter()
        .position(|e| matches!(e, Event::Erase(_)));
    let free = core.events.iter().position(|e| matches!(e, Event::Free(_)));
    assert!(erase.unwrap() < free.unwrap());
    assert_eq!(
        core.events[core.events.len() - 2..],
        [Event::Unlock, Event::Detach]
    );
    // A second clear neither locks nor frees again.
    core.events.clear();
    clear_mix(&mut core);
    assert_eq!(core.events, [Event::CanvasRead]);
    assert_eq!(core.freed.len(), 1);
    assert_eq!(core.list, rest);
}

// ---------------------------------------------------------------------------
// the C's cases

#[test]
fn characterization_no_mix() {
    // The early return reads nothing but the canvas's mix.
    let mut core = Core::default();
    clear_mix(&mut core);
    assert_eq!(core.events, [Event::CanvasRead]);
    let mut core = Core {
        list: vec![Some(7)],
        ..Core::default()
    };
    clear_mix(&mut core);
    assert_eq!(core.events, [Event::CanvasRead]);
    assert_eq!(core.list, [Some(7)]);
    assert!(core.freed.is_empty());
    // With the mutex held elsewhere, a lock would fail the recorder.
    core.held = true;
    core.events.clear();
    clear_mix(&mut core);
    assert_eq!(core.events, [Event::CanvasRead]);
    assert!(core.held && !core.locked);
}

#[test]
fn characterization_one_mix() {
    list_case(1, 0);
}

#[test]
fn characterization_several_mixes() {
    for position in 0..4 {
        list_case(4, position);
    }
}

#[test]
fn characterization_unregistered_mix() {
    // Characterized, not endorsed: an unregistered mix is detached, not freed.
    let mut core = Core {
        list: vec![Some(1)],
        canvas: Some(2),
        ..Core::default()
    };
    clear_mix(&mut core);
    assert_eq!(core.canvas, None);
    assert!(core.freed.is_empty());
    assert_eq!(core.list, [Some(1)]);
    assert_eq!(count_of(&core, Event::Lock), 1);
    assert_eq!(count_of(&core, Event::Unlock), 1);
}

#[test]
fn characterization_first_match_only() {
    // Characterized, not endorsed: the production list must not contain
    // duplicates.
    let mut core = Core {
        list: vec![Some(5), Some(5)],
        canvas: Some(5),
        ..Core::default()
    };
    clear_mix(&mut core);
    assert_eq!(core.freed, [Some(5)]);
    assert_eq!(core.list, [Some(5)]);
    assert_eq!(core.canvas, None);
}

// ---------------------------------------------------------------------------
// new edges

#[test]
fn edge_read_trace_of_the_c_loop() {
    use Event::{At, CanvasRead, Count, Detach, Erase, Free, Lock, Unlock};
    let mut core = Core {
        list: vec![Some(0), Some(1), Some(2), Some(3)],
        canvas: Some(2),
        ..Core::default()
    };
    clear_mix(&mut core);
    assert_eq!(
        core.events,
        [
            CanvasRead,
            Lock,
            Count,
            At(0),
            CanvasRead,
            Count,
            At(1),
            CanvasRead,
            Count,
            At(2),
            CanvasRead,
            Erase(2),
            Free(Some(2)),
            Unlock,
            Detach
        ]
    );
    // Not found: the length is read once more after the last entry.
    let mut core = Core {
        list: vec![Some(0)],
        canvas: Some(9),
        ..Core::default()
    };
    clear_mix(&mut core);
    assert_eq!(
        core.events,
        [
            CanvasRead,
            Lock,
            Count,
            At(0),
            CanvasRead,
            Count,
            Unlock,
            Detach
        ]
    );
    // An empty list: one length read.
    let mut core = Core {
        canvas: Some(1),
        ..Core::default()
    };
    clear_mix(&mut core);
    assert_eq!(core.events, [CanvasRead, Lock, Count, Unlock, Detach]);
}

/// The C, on plain values: the first entry equal to the canvas's mix is
/// erased and freed; the canvas is then cleared.
fn model(list: &[Option<u32>], canvas: Option<u32>) -> (Vec<Option<u32>>, Vec<Option<u32>>) {
    let mut list = list.to_vec();
    let mut freed = Vec::new();
    if canvas.is_some()
        && let Some(index) = list.iter().position(|entry| *entry == canvas)
    {
        freed.push(list.remove(index));
    }
    (list, freed)
}

proptest! {
    #[test]
    fn edge_any_list_matches_the_c(list in prop::collection::vec(prop::option::weighted(0.9, 0..6u32), 0..12),
        canvas in prop::option::of(0..8u32)) {
        let mut core = Core { list: list.clone(), canvas, ..Core::default() };
        clear_mix(&mut core);
        let (rest, freed) = model(&list, canvas);
        prop_assert_eq!(&core.list, &rest);
        prop_assert_eq!(&core.freed, &freed);
        prop_assert_eq!(core.canvas, None);
        prop_assert!(!core.locked);
        let locks = usize::from(canvas.is_some());
        prop_assert_eq!(count_of(&core, Event::Lock), locks);
        prop_assert_eq!(count_of(&core, Event::Unlock), locks);
        prop_assert_eq!(count_of(&core, Event::Detach), locks);
    }
}

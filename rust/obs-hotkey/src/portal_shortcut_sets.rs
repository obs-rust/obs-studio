//! Safe core of `libobs/obs-nix-portal-shortcut-sets.c.h`: the combination
//! set logic of the xdg-desktop-portal GlobalShortcuts client.
//!
//! On every sync of the hotkey bindings the portal client
//! (`obs-nix-portal-shortcuts.c`, Wayland with GIO, Linux and FreeBSD, still
//! C) asks this code:
//! - [`unchanged`]: whether the portal session stays as it is, or is bound
//!   again;
//! - [`request`]: which combinations the new bind asks the desktop for;
//! - [`partition`]: after the desktop's dialog is declined, which
//!   combinations are bound again (they were approved before) and which are
//!   declined;
//! - [`has`]: whether a set holds a combination (the Activated handler).
//!
//! It only reads the caller's arrays and returns positions in them, so the
//! C portal client keeps every allocation. A combination is any
//! [`Combination`] (the safe [`KeyCombination`], or the C struct in the
//! shim), and a description is any `T: PartialEq`, `None` for a NULL one;
//! the shim compares C strings as bytes, as `strcmp` does.
//!
//! Characterized, not endorsed (`test_portal_shortcut_sets`), and kept as
//! the C has them:
//! - the same set compares combinations as sets, so `{A, A}` is the same as
//!   `{A}`, and each copy is compared with the description of the first
//!   copy on the other side;
//! - a NULL description is not the same as another NULL;
//! - the partition counts duplicates in |part| and takes |request| as
//!   given, so two copies of an approved combination can stop the partial
//!   rebind.
//!
//! Every position written goes through a bounds check: the caller gives
//! room for `current.len()` positions, and a shorter buffer only loses the
//! positions past its end (C writes past it). The counts never depend on
//! the room.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

pub use crate::nix_hotkey_state::KeyCombination;

/// An element of a combination array: a key and its `INTERACT_*` bits.
pub trait Combination: Copy {
    /// The combination it holds.
    fn combination(self) -> KeyCombination;
}

impl Combination for KeyCombination {
    fn combination(self) -> KeyCombination {
        self
    }
}

/// `same_combo()` (`obs-nix-portal-shortcuts.h`): the same key and the same
/// modifier bits, every bit, not only the four the portal knows.
pub fn same_combo(a: KeyCombination, b: KeyCombination) -> bool {
    a.key == b.key && a.modifiers == b.modifiers
}

/// Whether `combos` holds `combo` (`obs_nix_portal_sets_has`).
pub fn has<C: Combination>(combos: &[C], combo: KeyCombination) -> bool {
    combos.iter().any(|c| same_combo(c.combination(), combo))
}

/// Every combination of `a` is also in `b`.
fn covered_by<A: Combination, B: Combination>(a: &[A], b: &[B]) -> bool {
    a.iter().all(|c| has(b, c.combination()))
}

/// The description of the first combination of `combos` that is `combo`,
/// among the ones that have a description (the C's `i < num` guards).
fn description_of<'a, C: Combination, T>(
    combos: &[C],
    descriptions: &'a [Option<T>],
    combo: KeyCombination,
) -> Option<&'a T> {
    combos
        .iter()
        .zip(descriptions)
        .find(|(c, _)| same_combo(c.combination(), combo))
        .and_then(|(_, d)| d.as_ref())
}

/// The same combinations, each with the same description. A combination
/// without a description, or with a NULL one, is never the same.
fn same_set<A: Combination, B: Combination, T: PartialEq>(
    a: &[A],
    a_descriptions: &[Option<T>],
    b: &[B],
    b_descriptions: &[Option<T>],
) -> bool {
    if !covered_by(a, b) || !covered_by(b, a) {
        return false;
    }
    a.iter().enumerate().all(|(i, c)| {
        let other = description_of(b, b_descriptions, c.combination());
        let mine = a_descriptions.get(i).and_then(Option::as_ref);
        matches!((mine, other), (Some(mine), Some(other)) if mine == other)
    })
}

/// Positions written in order into a caller's buffer, counted whether or
/// not the buffer has room for them.
struct Positions<'a> {
    out: &'a mut [usize],
    num: usize,
}

impl<'a> Positions<'a> {
    fn new(out: &'a mut [usize]) -> Self {
        Self { out, num: 0 }
    }

    fn push(&mut self, position: usize) {
        if let Some(slot) = self.out.get_mut(self.num) {
            *slot = position;
        }
        // At most one per element of a slice, so it cannot overflow.
        self.num = self.num.saturating_add(1);
    }
}

/// `select_combos()`: the positions of the combinations of `combos` that
/// are (`is_in`) or are not (`!is_in`) in `filter`, all of them without a
/// filter.
fn select<C: Combination, F: Combination>(
    combos: &[C],
    filter: Option<&[F]>,
    is_in: bool,
    out: &mut Positions<'_>,
) {
    for (i, c) in combos.iter().enumerate() {
        if filter.is_some_and(|f| has(f, c.combination()) != is_in) {
            continue;
        }
        out.push(i);
    }
}

/// `obs_nix_portal_sets_unchanged`: whether a sync leaves the portal session
/// as it is.
///
/// With a session: `current` holds the combinations of `bound`, each with
/// the same description (a changed description, an action moved to or from
/// a key, binds again so the desktop's list stays right). Without one:
/// `current` is empty, or every combination of it was declined before
/// (a declined set is not asked for again). Either way, `retry` of a
/// `partly_declined` session binds the full set anew.
#[allow(clippy::too_many_arguments)]
pub fn unchanged<C: Combination, T: PartialEq>(
    session: bool,
    retry: bool,
    partly_declined: bool,
    current: &[C],
    current_descriptions: &[Option<T>],
    bound: &[C],
    bound_descriptions: &[Option<T>],
    declined: &[C],
) -> bool {
    let unchanged = if session {
        same_set(current, current_descriptions, bound, bound_descriptions)
    } else {
        current.is_empty() || covered_by(current, declined)
    };
    if retry && session && partly_declined {
        return false;
    }
    unchanged
}

/// What [`request`] selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    /// The number of positions in the request.
    pub num: usize,
    /// It is only part of `current`.
    pub partly: bool,
}

/// `obs_nix_portal_sets_request`: the positions of the combinations of
/// `current` to ask the desktop for, in its order, into `request` (room for
/// `current.len()`): all of them, or while `partly_declined` those not in
/// `declined` (left out until Ask again, so the desktop does not ask for
/// them on every bind).
pub fn request<C: Combination>(
    current: &[C],
    partly_declined: bool,
    declined: &[C],
    request: &mut [usize],
) -> Request {
    let mut out = Positions::new(request);
    select(
        current,
        partly_declined.then_some(declined),
        false,
        &mut out,
    );
    Request {
        num: out.num,
        partly: out.num < current.len(),
    }
}

/// What [`partition`] selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Partition {
    /// The number of positions in part.
    pub num_part: usize,
    /// The number of positions in declined.
    pub num_declined: usize,
    /// Part is bound again: it is neither empty nor as large as the request.
    pub bind: bool,
}

/// `obs_nix_portal_sets_partition`: the desktop declined `request`. Part is
/// the combinations of `current` that it approved before and that are in
/// `request`; declined is the other ones of `current`. Both are positions
/// in `current`, in its order, each with room for `current.len()`.
///
/// As in C, `part` first receives every approved position and is then
/// filtered in place, so the entries past `num_part` (up to the number of
/// approved ones) hold approved positions that were filtered out.
pub fn partition<C: Combination>(
    current: &[C],
    approved: &[C],
    request: &[C],
    part: &mut [usize],
    declined: &mut [usize],
) -> Partition {
    let mut selected = Positions::new(part);
    select(current, Some(approved), true, &mut selected);
    // The approved positions, filtered by request in place. The C reads
    // them back from part; walking current again gives the same positions
    // in the same order. A combination of current is in part exactly when
    // it was approved and requested (its own position is then in part).
    let in_part = |c: KeyCombination| has(approved, c) && has(request, c);
    let mut kept = Positions::new(selected.out);
    for (i, c) in current.iter().enumerate() {
        if in_part(c.combination()) {
            kept.push(i);
        }
    }
    let num_part = kept.num;

    let mut rest = Positions::new(declined);
    for (i, c) in current.iter().enumerate() {
        if !in_part(c.combination()) {
            rest.push(i);
        }
    }
    Partition {
        num_part,
        num_declined: rest.num,
        bind: num_part > 0 && num_part < request.len(),
    }
}

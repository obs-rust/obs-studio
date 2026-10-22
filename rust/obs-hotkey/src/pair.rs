//! Safe core of `libobs/obs-hotkey-pair.c.h`: the hotkey pair press latch.
//!
//! A hotkey pair (`obs_hotkey_pair_register_*()`, such as Start and Stop
//! Streaming) is two hotkeys, each with a func that returns whether it
//! acted. libobs registers each half with its own callback, and each half
//! keeps a latch (`pressed0` and `pressed1` of the pair):
//! - while one half is latched, every event of the other half is dropped;
//! - a press whose func returns true latches its half; its release then
//!   clears the latch without calling the func;
//! - any other event calls the func, and if it returns true the latch takes
//!   the event's pressed value.
//!
//! The callbacks run with the hotkey mutex held: on the hotkey thread, in
//! `obs_hotkey_inject_event()`, or in `obs_hotkey_trigger_routed_callback()`
//! (the frontend's router and obs-websocket's `TriggerHotkeyByName`). The
//! func runs inside the callback and may trigger the other half, or this
//! one, synchronously. So [`event`] reads and writes the latches only
//! through [`Latches`], at the C's points: the other half's latch, then this
//! half's, before the func; this half's latch, and only it, after the func
//! returns. It never writes back a value read before the call, and never
//! touches the other half's latch. It keeps no state and takes no lock.
//!
//! Characterized, not endorsed (`test_hotkey_pair_latch`,
//! `test_hotkey_pair_steps`), and kept as the C has them:
//! - a release without a press reaches the func (the second half pressed
//!   while the first is latched, and released after it; one key bound to
//!   both halves does this on every press);
//! - a second press of a latched half calls the func again, and the half
//!   stays latched whatever the func returns;
//! - both halves can end up latched (a func that triggers its partner's
//!   press from inside its own), and the pair then drops every event.

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

/// One half of a pair: the first (`func[0]`, `pressed0`) or the second
/// (`func[1]`, `pressed1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Half {
    First,
    Second,
}

impl Half {
    /// Both halves, in the C's order.
    pub const BOTH: [Half; 2] = [Half::First, Half::Second];

    /// The other half: the one whose latch blocks this one.
    pub const fn other(self) -> Half {
        match self {
            Half::First => Half::Second,
            Half::Second => Half::First,
        }
    }

    /// The C's `half` index (`obs_hotkey_pair_call_func()`): 0 or 1.
    pub const fn index(self) -> usize {
        match self {
            Half::First => 0,
            Half::Second => 1,
        }
    }
}

/// The two latches of one pair, as the latch reads and writes them. The C
/// reaches them through `obs_hotkey_pair_fields()`; a func may change
/// either one while it runs.
pub trait Latches {
    /// Whether `half` is latched.
    fn latched(&self, half: Half) -> bool;
    /// Sets the latch of `half`.
    fn set_latched(&mut self, half: Half, latched: bool);
}

/// Plain latches: `pressed0` and `pressed1` held by value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PairLatches {
    /// `pressed0`.
    pub first: bool,
    /// `pressed1`.
    pub second: bool,
}

impl Latches for PairLatches {
    fn latched(&self, half: Half) -> bool {
        match half {
            Half::First => self.first,
            Half::Second => self.second,
        }
    }

    fn set_latched(&mut self, half: Half, latched: bool) {
        match half {
            Half::First => self.first = latched,
            Half::Second => self.second = latched,
        }
    }
}

/// What one event did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The other half was latched: nothing was called or written.
    Dropped,
    /// This half was latched and the event was a release: its latch was
    /// cleared, without calling the func.
    Cleared,
    /// The func was called and returned this; if true, this half's latch
    /// was then set to the event's pressed value.
    Called(bool),
}

/// `obs_hotkey_pair_first_func()` (`half` First) or
/// `obs_hotkey_pair_second_func()` (`half` Second): one event of one half.
///
/// `call` is the half's func (`obs_hotkey_pair_call_func()`), called at
/// most once. It gets the latches, as a C func can reach them by
/// triggering either half from inside its call; whatever it leaves there
/// is kept, except that a true result then sets this half's latch.
pub fn event<L: Latches + ?Sized>(
    latches: &mut L,
    half: Half,
    pressed: bool,
    call: impl FnOnce(&mut L) -> bool,
) -> Outcome {
    if latches.latched(half.other()) {
        return Outcome::Dropped;
    }
    if latches.latched(half) && !pressed {
        latches.set_latched(half, false);
        return Outcome::Cleared;
    }
    // The latch is written after the func returns, and only this half's:
    // the func may have changed either one.
    let acted = call(latches);
    if acted {
        latches.set_latched(half, pressed);
    }
    Outcome::Called(acted)
}

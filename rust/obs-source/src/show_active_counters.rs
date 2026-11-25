//! `obs_source_inc_showing()`, `obs_source_inc_active()`,
//! `obs_source_dec_showing()` and `obs_source_dec_active()`: each checks its
//! source, then hands it to `obs_source_activate()` or
//! `obs_source_deactivate()` (both stay C) with an auxiliary or main view.
use core::ffi::CStr;

/// `MAIN_VIEW` of the private `enum view_type` (`obs-internal.h`).
pub const MAIN_VIEW: u32 = 1;
/// `AUX_VIEW` of the private `enum view_type` (`obs-internal.h`).
pub const AUX_VIEW: u32 = 2;

/// Which of the four public counters was called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// `obs_source_inc_showing()`.
    IncShowing,
    /// `obs_source_inc_active()`.
    IncActive,
    /// `obs_source_dec_showing()`.
    DecShowing,
    /// `obs_source_dec_active()`.
    DecActive,
}

/// The C call a change forwards to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// `obs_source_activate()`.
    Activate,
    /// `obs_source_deactivate()`.
    Deactivate,
}

impl Change {
    /// All four, in header order.
    pub const ALL: [Self; 4] = [
        Self::IncShowing,
        Self::IncActive,
        Self::DecShowing,
        Self::DecActive,
    ];

    /// The public function, named in its NULL diagnostic.
    #[expect(clippy::todo, reason = "RED stub for #5601")]
    pub fn function(self) -> &'static CStr {
        todo!("#5601")
    }

    /// The C call this change forwards to.
    #[expect(clippy::todo, reason = "RED stub for #5601")]
    pub fn direction(self) -> Direction {
        todo!("#5601")
    }

    /// The view type handed to that call.
    #[expect(clippy::todo, reason = "RED stub for #5601")]
    pub fn view_type(self) -> u32 {
        todo!("#5601")
    }
}

/// `None` is a NULL source: it logs once with the function's name and calls
/// nothing. Otherwise `forward` is called once with the source, the C call
/// and the view type, and nothing is logged.
#[expect(clippy::todo, reason = "RED stub for #5601")]
pub fn apply<S>(
    _change: Change,
    _source: Option<S>,
    _forward: impl FnOnce(S, Direction, u32),
    _log: impl FnOnce(&'static CStr),
) {
    todo!("#5601")
}

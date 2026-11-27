//! `obs_source_inc_showing()`, `obs_source_inc_active()`,
//! `obs_source_dec_showing()` and `obs_source_dec_active()`: each checks its
//! source, then hands it to `obs_source_activate()` or
//! `obs_source_deactivate()` ([`crate::activate`], reached through the C
//! ABI) with an auxiliary or main view.
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
    pub fn function(self) -> &'static CStr {
        match self {
            Self::IncShowing => c"obs_source_inc_showing",
            Self::IncActive => c"obs_source_inc_active",
            Self::DecShowing => c"obs_source_dec_showing",
            Self::DecActive => c"obs_source_dec_active",
        }
    }

    /// The C call this change forwards to.
    pub fn direction(self) -> Direction {
        match self {
            Self::IncShowing | Self::IncActive => Direction::Activate,
            Self::DecShowing | Self::DecActive => Direction::Deactivate,
        }
    }

    /// The view type handed to that call.
    pub fn view_type(self) -> u32 {
        match self {
            Self::IncShowing | Self::DecShowing => AUX_VIEW,
            Self::IncActive | Self::DecActive => MAIN_VIEW,
        }
    }
}

/// `None` is a NULL source: it logs once with the function's name and calls
/// nothing. Otherwise `forward` is called once with the source, the C call
/// and the view type, and nothing is logged.
pub fn apply<S>(
    change: Change,
    source: Option<S>,
    forward: impl FnOnce(S, Direction, u32),
    log: impl FnOnce(&'static CStr),
) {
    match source {
        Some(source) => forward(source, change.direction(), change.view_type()),
        None => log(change.function()),
    }
}

//! Lazy output activity queries; the caller owns each atomic load.

/// A flag requested in the original C short-circuit order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    Active,
    Starting,
    Reconnecting,
    DelayActive,
}

/// Whether encoders are in use or about to be used.
pub fn needs_encoders(_load: impl FnMut(Flag) -> bool) -> bool {
    todo!()
}

/// Whether encoder use is about to start; does not read active.
pub fn starting(_load: impl FnMut(Flag) -> bool) -> bool {
    todo!()
}

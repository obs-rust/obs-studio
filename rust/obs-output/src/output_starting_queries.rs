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
pub fn needs_encoders(mut load: impl FnMut(Flag) -> bool) -> bool {
    load(Flag::Active)
        || load(Flag::Starting)
        || load(Flag::Reconnecting)
        || load(Flag::DelayActive)
}

/// Whether encoder use is about to start; does not read active.
pub fn starting(mut load: impl FnMut(Flag) -> bool) -> bool {
    load(Flag::Starting) || load(Flag::Reconnecting) || load(Flag::DelayActive)
}

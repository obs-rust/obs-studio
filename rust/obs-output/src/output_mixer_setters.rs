//! Safe mixer setter decisions, preserving the original gate order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Null,
    Encoded,
    InvalidIndex,
    Active,
    Set(usize),
}
pub fn decide(
    present: bool,
    encoded: bool,
    index: Option<usize>,
    active: impl FnOnce() -> bool,
    mask: usize,
) -> Decision {
    if !present {
        return Decision::Null;
    }
    if encoded {
        return Decision::Encoded;
    }
    if index.is_some_and(|i| !crate::output_slots::mixer_in_range(i)) {
        return Decision::InvalidIndex;
    }
    if active() {
        return Decision::Active;
    }
    Decision::Set(index.map_or(mask, crate::output_slots::mixer_mask))
}

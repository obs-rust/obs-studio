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
    todo!()
}

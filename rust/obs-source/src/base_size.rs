//! Base-size traversal, shared by the public queries and C rendering callers.
use crate::size_route::{self, Node, Route};

/// Axis requested by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Width,
    Height,
}

/// Borrowed source operations under the caller's original synchronization.
pub trait Source: Copy {
    fn has_data(self) -> bool;
    fn node(self, axis: Axis) -> Node;
    fn transition_size(self, axis: Axis) -> u32;
    fn callback_size(self, axis: Axis) -> u32;
    fn target(self) -> Option<Self>;
    fn async_size(self, axis: Axis) -> u32;
}

/// Public queries require context data on their initial source only.
pub fn query<S: Source>(source: Option<S>, axis: Axis, mut null_log: impl FnMut(Axis)) -> u32 {
    todo!()
}

/// Internal traversal used by rendering and recursive filter-target queries.
pub fn base<S: Source>(source: S, axis: Axis) -> u32 {
    todo!()
}

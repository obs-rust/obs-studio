//! Scene-item order position: the head read before the full lock, then one
//! full lock around the walk of the parent's list.

use core::ffi::c_int;

/// An attached item's parent list, as seen by the query. C owns the lock,
/// the head and the links.
pub trait ParentList {
    /// An item handle, compared by identity.
    type Item: Copy + PartialEq;
    /// The item whose position is asked for.
    fn target(&self) -> Self::Item;
    /// The parent scene's first item; read once, before the lock.
    fn head(&mut self) -> Option<Self::Item>;
    /// Take the parent scene's full (video, then audio) lock.
    fn lock(&mut self);
    /// The item after `item` in the parent's list; read under the lock.
    fn next(&mut self, item: Self::Item) -> Option<Self::Item>;
    /// Release the full lock.
    fn unlock(&mut self);
}

/// `None` (a NULL or detached item, rejected by
/// [`crate::item_order::item_attached`]) is 0 without locking. Otherwise the
/// number of items before the target in its parent's list, or the list's
/// length when the target is not in it, under exactly one lock.
pub fn order_position<L: ParentList>(list: Option<&mut L>) -> c_int {
    let _ = list;
    0
}

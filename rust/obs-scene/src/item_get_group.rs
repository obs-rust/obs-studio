//! Parent-group query: reject before locking, then one full lock around the walk.

/// A present scene and item, as seen by the query. C owns the locks and walk.
pub trait Query {
    /// The group item the walk returns, borrowed.
    type Group;
    /// Read the item's group flag; called before any lock.
    fn is_group(&mut self) -> bool;
    /// Take the scene's full (video, then audio) lock.
    fn lock(&mut self);
    /// The first scene item that is the group holding the item, if any.
    fn lookup(&mut self) -> Option<Self::Group>;
    /// Release the full lock.
    fn unlock(&mut self);
}

/// A missing scene or item, or a group item, is `None` without locking.
/// Otherwise the walk's result is returned as is, under exactly one lock.
pub fn get_group<Q: Query>(query: Option<&mut Q>) -> Option<Q::Group> {
    let _ = query;
    None // RED stub: the query is not ported yet.
}

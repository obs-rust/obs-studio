//! Ordered scene lookup. A recursive name query visits each group's immediate
//! children before the next top-level item, exactly as the original C does.

/// Access to live scene lists. Implementations keep each list locked throughout
/// `with_scene`, including any child lookup performed by its closure.
pub trait Access {
    type Scene: Copy;
    type Item: Copy;
    fn with_scene<R>(&self, scene: Self::Scene, visit: impl FnOnce(Option<Self::Item>) -> R) -> R;
    fn next(&self, item: Self::Item) -> Option<Self::Item>;
    fn name(&self, item: Self::Item) -> Option<&[u8]>;
    fn id(&self, item: Self::Item) -> i64;
    fn is_group(&self, item: Self::Item) -> bool;
    fn group(&self, item: Self::Item) -> Option<Self::Scene>;
}

/// Lookup mode; names are C-string bytes without their terminator.
#[derive(Clone, Copy)]
pub enum Query<'a> {
    Name(&'a [u8]),
    RecursiveName(&'a [u8]),
    Id(i64),
    Group(Option<&'a [u8]>),
}

/// Return the original first matching item, without acquiring a reference.
pub fn find<A: Access>(_access: &A, _scene: Option<A::Scene>, _query: Query<'_>) -> Option<A::Item> {
    todo!("scene lookup port")
}

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
pub fn find<A: Access>(access: &A, scene: Option<A::Scene>, query: Query<'_>) -> Option<A::Item> {
    let scene = scene?;
    if matches!(query, Query::Group(None | Some([]))) {
        return None;
    }
    access.with_scene(scene, |mut item| {
        while let Some(current) = item {
            let matches = match query {
                Query::Name(name) | Query::RecursiveName(name) => {
                    access.name(current) == Some(name)
                }
                Query::Id(id) => access.id(current) == id,
                Query::Group(name) => access.is_group(current) && access.name(current) == name,
            };
            if matches {
                return Some(current);
            }
            if let Query::RecursiveName(name) = query
                && access.is_group(current)
                && let Some(child) = find(access, access.group(current), Query::Name(name))
            {
                return Some(child);
            }
            item = access.next(current);
        }
        None
    })
}

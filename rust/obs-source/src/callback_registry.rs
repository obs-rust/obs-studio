//! Ordered (callback, param) subscriptions shared by the source callback
//! lists; duplicates and null tokens are retained, removal erases the first
//! exact pair.
use core::ffi::CStr;

pub trait Registry {
    type Pair: Copy + Eq;
    fn count(&self) -> usize;
    fn pair(&self, index: usize) -> Self::Pair;
    fn append(&mut self, pair: Self::Pair);
    fn erase(&mut self, index: usize);
}

/// `names` are the public add and remove entry names the NULL-source
/// diagnostic reports, in that order.
pub fn update<R: Registry>(
    registry: Option<&mut R>,
    pair: R::Pair,
    add: bool,
    names: [&'static CStr; 2],
    log: impl FnOnce(&'static CStr),
) {
    let Some(registry) = registry else {
        log(if add { names[0] } else { names[1] });
        return;
    };
    if add {
        registry.append(pair);
    } else if let Some(index) = (0..registry.count()).find(|&i| registry.pair(i) == pair) {
        registry.erase(index);
    }
}

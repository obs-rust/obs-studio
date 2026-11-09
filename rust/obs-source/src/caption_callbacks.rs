//! Ordered caption subscriptions; duplicates and null tokens are retained.
use core::ffi::CStr;

pub trait Registry {
    type Pair: Copy + Eq;
    fn count(&self) -> usize;
    fn pair(&self, index: usize) -> Self::Pair;
    fn append(&mut self, pair: Self::Pair);
    fn erase(&mut self, index: usize);
}

pub fn update<R: Registry>(
    registry: Option<&mut R>,
    pair: R::Pair,
    add: bool,
    log: impl FnOnce(&'static CStr),
) {
    let Some(registry) = registry else {
        log(if add {
            c"obs_source_add_caption_callback"
        } else {
            c"obs_source_remove_caption_callback"
        });
        return;
    };
    if add {
        registry.append(pair);
    } else if let Some(index) = (0..registry.count()).find(|&i| registry.pair(i) == pair) {
        registry.erase(index);
    }
}

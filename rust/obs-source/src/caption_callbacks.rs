//! Ordered caption subscriptions; duplicates and null tokens are retained.
use core::ffi::CStr;

pub trait Registry {
    type Pair: Copy + Eq;
    fn count(&self) -> usize;
    fn pair(&self, index: usize) -> Self::Pair;
    fn append(&mut self, pair: Self::Pair);
    fn erase(&mut self, index: usize);
}

pub fn update<R: Registry>(registry: Option<&mut R>, pair: R::Pair, add: bool, log: impl FnOnce(&'static CStr)) {
    let _ = (registry, pair, add, log);
    todo!()
}

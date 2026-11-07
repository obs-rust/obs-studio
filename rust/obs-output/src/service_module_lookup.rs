//! Read-only service module lookup rules (#3839).

/// Select the enabled or disabled module list without changing either list.
pub fn first<T: Copy>(disabled: bool, enabled: T, unavailable: T) -> T {
    if disabled { unavailable } else { enabled }
}

/// A borrowed module view; ownership and list lifetime stay with libobs.
pub struct Module<T> {
    pub next: T,
    pub service_count: usize,
    pub load_state: i32,
}
impl<T: Copy> Module<T> {
    pub fn next(&self) -> T {
        self.next
    }
    pub fn service_count(&self) -> usize {
        self.service_count
    }
    pub fn load_state(&self) -> i32 {
        self.load_state
    }
}

/// Return an id from the caller's valid prefix of registered services.
pub fn service_id<T: Copy>(ids: &[T], index: usize) -> T {
    ids[index]
}

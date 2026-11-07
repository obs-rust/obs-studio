//! Read-only service module lookup rules (#3839).

/// Select the enabled or disabled module list without changing either list.
pub fn first<T: Copy>(disabled: bool, enabled: T, unavailable: T) -> T {
    let _ = (disabled, enabled, unavailable);
    todo!("#3839")
}

/// A borrowed module view; ownership and list lifetime stay with libobs.
pub struct Module<'a, T> {
    pub next: T,
    pub services: &'a [T],
    pub load_state: i32,
}
impl<T: Copy> Module<'_, T> {
    pub fn next(&self) -> T { todo!("#3839") }
    pub fn service_count(&self) -> usize { todo!("#3839") }
    pub fn service_id(&self, index: usize) -> T { let _ = index; todo!("#3839") }
    pub fn load_state(&self) -> i32 { todo!("#3839") }
}

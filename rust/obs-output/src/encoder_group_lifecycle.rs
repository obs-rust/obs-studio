//! Encoder-group ownership and the locked teardown handoff.
//!
//! The backend owns allocation, private C fields and native mutex primitives.
//! All lifecycle ordering and deferred destruction decisions live here.
pub trait Backend {
    type Group: Copy;
    fn allocate_zeroed(&mut self) -> Self::Group;
    fn init_mutex(&mut self, group: Self::Group) -> bool;
    fn free_group(&mut self, group: Self::Group);
    fn lock(&mut self, group: Self::Group);
    fn unlock(&mut self, group: Self::Group);
    fn destroy_mutex(&mut self, group: Self::Group);
    fn started(&mut self, group: Self::Group) -> u32;
    fn defer(&mut self, group: Self::Group);
    fn count(&mut self, group: Self::Group) -> usize;
    fn clear_back_pointer(&mut self, group: Self::Group, index: usize);
    fn release(&mut self, group: Self::Group, index: usize);
    fn free_list(&mut self, group: Self::Group);
}

pub fn create<B: Backend>(backend: &mut B) -> Option<B::Group> {
    let group = backend.allocate_zeroed();
    if backend.init_mutex(group) {
        Some(group)
    } else {
        backend.free_group(group);
        None
    }
}

/// The caller hands over a group whose mutex is already locked.
pub fn actually_destroy<B: Backend>(backend: &mut B, group: B::Group) {
    let mut index = 0;
    while index < backend.count(group) {
        backend.clear_back_pointer(group, index);
        backend.release(group, index);
        index += 1;
    }
    backend.free_list(group);
    backend.unlock(group);
    backend.destroy_mutex(group);
    backend.free_group(group);
}

pub fn destroy<B: Backend>(backend: &mut B, group: Option<B::Group>) {
    let Some(group) = group else {
        return;
    };
    backend.lock(group);
    if backend.started(group) != 0 {
        backend.defer(group);
        backend.unlock(group);
        return;
    }
    actually_destroy(backend, group);
}

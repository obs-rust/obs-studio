//! Ordered hotkey removal. Storage and callbacks belong to the caller.

/// Operations on a locked hotkey registry. Handles stay live until `free`.
pub trait Registry {
    /// A key borrowed from the registry.
    type Key: Copy;
    /// Next allocated identifier.
    fn next_id(&self) -> usize;
    /// Find a currently registered key.
    fn find(&mut self, id: usize) -> Option<Self::Key>;
    /// Remove only the hash entry, leaving the object alive.
    fn remove(&mut self, key: Self::Key);
    /// Notify observers while strings and owner remain live.
    fn notify(&mut self, key: Self::Key);
    /// C registerer enum, read at the point of use.
    fn category(&self, key: Self::Key) -> i32;
    /// Release the category's weak owner (including a null owner).
    fn release(&mut self, key: Self::Key, category: i32);
    /// Clear the stored owner pointer.
    fn clear_owner(&mut self, key: Self::Key);
    /// Remove bindings, releasing pressed bindings before object destruction.
    fn remove_bindings(&mut self, id: usize);
    /// Free the key's name.
    fn free_name(&mut self, key: Self::Key);
    /// Free the key's description.
    fn free_description(&mut self, key: Self::Key);
    /// Free the detached key itself.
    fn free(&mut self, key: Self::Key);
}

/// Release the weak owner, then clear the pointer even for frontend keys.
pub fn release_registerer<R: Registry>(registry: &mut R, key: R::Key) {
    let category = registry.category(key);
    if (1..=4).contains(&category) {
        registry.release(key, category);
    }
    registry.clear_owner(key);
}

/// Remove an existing key; future and already removed identifiers do nothing.
pub fn unregister_hotkey<R: Registry>(registry: &mut R, id: usize) {
    if id >= registry.next_id() {
        return;
    }
    let Some(key) = registry.find(id) else {
        return;
    };
    registry.remove(key);
    registry.notify(key);
    release_registerer(registry, key);
    // Characterized, not endorsed: the C releases the cleared source pointer.
    if registry.category(key) == 1 {
        registry.release(key, 1);
    }
    registry.remove_bindings(id);
    registry.free_name(key);
    registry.free_description(key);
    registry.free(key);
}

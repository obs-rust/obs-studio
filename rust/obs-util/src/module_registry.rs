//! Shared enabled/disabled module registry traversal for source, output and encoder IDs.

/// Borrowed registry access. The adapter chooses the type-specific ID array.
pub trait Registry {
    type Handle: Copy;
    fn first(&self, disabled: bool) -> Option<Self::Handle>;
    fn next(&self, module: Self::Handle) -> Option<Self::Handle>;
    fn id_count(&self, module: Self::Handle) -> usize;
    fn id(&self, module: Self::Handle, index: usize) -> &[u8];
    fn load_state(&self, module: Self::Handle) -> i32;
}

/// Search enabled modules before disabled modules, preserving list and ID order.
/// IDs are C-string bytes before their first NUL, without UTF-8 conversion.
pub fn find<R: Registry>(registry: &R, id: Option<&[u8]>) -> Option<R::Handle> {
    for disabled in [false, true] {
        let mut current = registry.first(disabled);
        while let Some(module) = current {
            for i in 0..registry.id_count(module) {
                if Some(registry.id(module, i)) == id {
                    return Some(module);
                }
            }
            current = registry.next(module);
        }
    }
    None
}

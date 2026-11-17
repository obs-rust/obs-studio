//! Replace owned registry strings, preserving the C free-before-duplicate order.

/// The selected identity string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Description,
}

/// Storage and allocation at the registry boundary.
pub trait Registry {
    type Input;
    type Slot;
    type Owned;
    fn find(&mut self, id: usize, field: Field) -> Option<Self::Slot>;
    fn release(&mut self, slot: &Self::Slot);
    fn duplicate(&mut self, input: Self::Input) -> Self::Owned;
    fn store(&mut self, slot: Self::Slot, value: Self::Owned);
}

/// Unknown IDs do not inspect the input or allocate. NULL input clears a slot.
pub fn set<R: Registry>(registry: &mut R, id: usize, field: Field, input: R::Input) {
    if let Some(slot) = registry.find(id, field) {
        registry.release(&slot);
        let value = registry.duplicate(input);
        registry.store(slot, value);
    }
}

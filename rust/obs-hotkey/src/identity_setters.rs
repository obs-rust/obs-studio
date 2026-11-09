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
pub fn set<R: Registry>(_registry: &mut R, _id: usize, _field: Field, _input: R::Input) {
    todo!()
}

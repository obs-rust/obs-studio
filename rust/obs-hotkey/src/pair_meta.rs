//! Paired metadata updates through the existing single-hotkey setters.

/// The metadata field selected by a paired update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Description,
}

/// Registry-owned pair tokens and values; no additional lock is taken.
pub trait Registry {
    type Pair;
    type Value: Copy;
    fn find(&mut self, id: usize) -> Option<Self::Pair>;
    fn hotkey_id(&self, pair: &Self::Pair, half: usize) -> usize;
    fn set(&mut self, id: usize, value: Self::Value, field: Field);
}

/// Update half zero before half one, reading each ID immediately before its setter.
pub fn set<R: Registry>(registry: &mut R, id: usize, values: [R::Value; 2], field: Field) {
    let Some(pair) = registry.find(id) else {
        return;
    };
    for (half, value) in values.into_iter().enumerate() {
        let hotkey = registry.hotkey_id(&pair, half);
        registry.set(hotkey, value, field);
    }
}

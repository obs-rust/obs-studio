//! Position, rotation and scale writes under the captured scene lock.
use crate::item_bounds_setters::Fields;

/// The requested field and its caller-supplied value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Position([f32; 2]),
    Rotation(f32),
    Scale([f32; 2]),
}

/// Live fields or a safe model. Input is read only after acquiring the lock.
pub trait Item: Fields {
    fn input(&self) -> Value;
    fn absolute(&self) -> bool;
    /// Existing scene/canvas conversion, including group and nested-scene rules.
    fn relative(&mut self, value: Value) -> Value;
    fn store(&mut self, value: Value);
}

/// NULL items do nothing; every live write updates, including unchanged values.
pub fn set<I: Item>(item: Option<&mut I>) {
    let Some(item) = item else { return };
    crate::item_bounds_setters::set(item, |item| {
        let value = item.input();
        let stored = if !matches!(value, Value::Rotation(_)) && !item.absolute() {
            item.relative(value)
        } else {
            value
        };
        item.store(stored);
    });
}

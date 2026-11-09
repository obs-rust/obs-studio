//! Borrow the core's monitoring device fields into independently optional outputs.

/// The two borrowed monitoring device fields.
#[derive(Clone, Copy)]
pub enum Field {
    Name,
    Id,
}

/// Read only fields whose outputs are requested, without retaining their values.
pub fn get<T: Copy>(name: Option<&mut T>, id: Option<&mut T>, mut read: impl FnMut(Field) -> T) {
    if let Some(name) = name {
        *name = read(Field::Name);
    }
    if let Some(id) = id {
        *id = read(Field::Id);
    }
}

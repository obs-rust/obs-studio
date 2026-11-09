//! Borrow the core's monitoring device fields into independently optional outputs.

/// The two borrowed monitoring device fields.
#[derive(Clone, Copy)]
pub enum Field {
    Name,
    Id,
}

/// Read only fields whose outputs are requested, without retaining their values.
pub fn get<T: Copy>(_name: Option<&mut T>, _id: Option<&mut T>, _read: impl FnMut(Field) -> T) {
    todo!()
}

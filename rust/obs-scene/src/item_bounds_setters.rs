//! Bounds writes and their lock/write/unlock/update ordering.
use obs_graphics::vec2::Vec2;

/// A requested bounds field write.
#[derive(Clone, Copy, Debug)]
pub enum Write {
    Type(i64),
    Alignment(u32),
    Crop(bool),
    Size(Vec2),
}

/// Existing scene dependencies, supplied by the C boundary.
pub trait Target {
    fn lock(&mut self);
    fn write(&mut self, value: Write);
    fn unlock(&mut self);
    fn update(&mut self);
}

/// Unknown enum values mean no bounds, including untruncated scene-load values.
pub fn valid_bounds_type(value: i64) -> u32 {
    todo!()
}

/// Convert a size only when the item uses relative coordinates.
pub fn bounds_size(value: Vec2, absolute: bool, height: f32) -> Vec2 {
    todo!()
}

/// A missing item is a no-op. Updates occur after releasing the field lock.
pub fn apply<T: Target>(target: Option<&mut T>, value: Write) {
    todo!()
}

//! Argument construction before the existing immediate-mode vector boundary.
//! Context diagnostics, array routing and capacity checks stay at that boundary.
use crate::{vec2::Vec2, vec3::Vec3};

/// Construct a vertex with a positive-zero auxiliary lane.
pub fn vertex3f(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Construct a texture coordinate and preserve the full signed texture unit.
pub fn texcoord(x: f32, y: f32, unit: i32) -> (Vec2, i32) {
    (Vec2::new(x, y), unit)
}

/// Extend a two-dimensional vertex with positive-zero z and auxiliary lanes.
pub fn vertex2v(value: Vec2) -> Vec3 {
    crate::immediate_scalars::vertex2f(value.x, value.y)
}

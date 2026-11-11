//! Argument construction before the existing immediate-mode vector boundary.
//! Context diagnostics, array routing and capacity checks stay at that boundary.
use crate::{vec2::Vec2, vec3::Vec3};

/// Construct a vertex with a positive-zero auxiliary lane.
pub fn vertex3f(_x: f32, _y: f32, _z: f32) -> Vec3 {
    todo!()
}

/// Construct a texture coordinate and preserve the full signed texture unit.
pub fn texcoord(_x: f32, _y: f32, _unit: i32) -> (Vec2, i32) {
    todo!()
}

/// Extend a two-dimensional vertex with positive-zero z and auxiliary lanes.
pub fn vertex2v(_value: Vec2) -> Vec3 {
    todo!()
}

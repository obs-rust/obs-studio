//! Scalar immediate-mode inputs, before the existing vector submission boundary.
use crate::vec3::Vec3;

/// Construct a two-dimensional vertex with positive-zero z and auxiliary lane.
pub fn vertex2f(x: f32, y: f32) -> Vec3 {
    let _ = (x, y);
    todo!("port gs_vertex2f")
}

/// Construct a normal without arithmetic on its component bits.
pub fn normal3f(x: f32, y: f32, z: f32) -> Vec3 {
    let _ = (x, y, z);
    todo!("port gs_normal3f")
}

//! Normalize source rotations without overflowing at signed integer extremes.

/// Return the equivalent angle in 0..360.
pub fn normalize(rotation: i64) -> i64 {
    rotation.rem_euclid(360)
}

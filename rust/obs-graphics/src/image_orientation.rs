//! Selection of decoded image orientation. FFmpeg access stays at the FFI boundary.

/// Prefer legacy case-sensitive metadata, otherwise validate and copy a matrix.
/// The converter is FFmpeg's EXIF API on newer versions and the existing sign
/// table on older versions. Closures preserve the metadata early return.
pub fn orientation(
    metadata: impl FnOnce() -> Option<i32>,
    matrix: impl FnOnce() -> Option<[i32; 9]>,
    convert: impl FnOnce(&[i32; 9]) -> i32,
) -> i32 {
    let _ = (metadata, matrix, convert);
    todo!()
}

/// Read exactly nine native-endian matrix elements, including unaligned data.
pub fn matrix_bytes(bytes: &[u8]) -> Option<[i32; 9]> {
    let _ = bytes;
    todo!()
}

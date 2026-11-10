//! macOS: the loaded images from dyld, in load order.

use crate::images::Image;

pub const IMAGES_SUPPORTED: bool = true;

/// The loaded images in load order; the main program first.
pub fn images() -> Vec<Image> {
    todo!()
}

//! Hosts where the image list is not built yet (#4065 step 4).

use crate::images::Image;

pub const IMAGES_SUPPORTED: bool = false;

/// No images: the watchdog line then names no library.
pub fn images() -> Vec<Image> {
    Vec::new()
}

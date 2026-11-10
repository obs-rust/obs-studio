//! Hosts where the image list and the stack capture are not built yet
//! (#4065 step 4): the watchdog line then names no library.

use crate::images::Image;

pub const IMAGES_SUPPORTED: bool = false;
pub const STACK_SUPPORTED: bool = false;

/// No images.
pub fn images() -> Vec<Image> {
    Vec::new()
}

/// No thread to capture.
pub struct Thread;

pub fn current_thread() -> Thread {
    Thread
}

pub fn prepare_stack() {}

/// No frames.
pub fn capture_stack(_thread: &Thread, _out: &mut [usize]) -> usize {
    0
}

//! Canvas strong release. A NULL canvas is a silent no-op, even after
//! shutdown; any other canvas follows the common object release order.
use crate::object_ref::{self, Host};

/// Release one strong canvas owner; `present` is false for a NULL handle.
pub fn release<H: Host>(present: bool, host: &mut H) {
    if present {
        object_ref::release(host);
    }
}

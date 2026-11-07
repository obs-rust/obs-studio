//! Read-only output pause capability query.
use crate::NullLog;

/// `OBS_OUTPUT_CAN_PAUSE` from `obs-output.h`.
pub const OBS_OUTPUT_CAN_PAUSE: u32 = 1 << 5;

/// Query the current output flags; a missing output logs and returns false.
pub fn can_pause(_flags: Option<u32>, _log: &impl NullLog) -> bool {
    todo!("#3895")
}

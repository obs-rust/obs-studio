//! Read-only output pause capability query.
use crate::NullLog;

/// `OBS_OUTPUT_CAN_PAUSE` from `obs-output.h`.
pub const OBS_OUTPUT_CAN_PAUSE: u32 = 1 << 5;

/// Query the current output flags; a missing output logs and returns false.
pub fn can_pause(flags: Option<u32>, log: &impl NullLog) -> bool {
    match flags {
        Some(flags) => flags & OBS_OUTPUT_CAN_PAUSE != 0,
        None => {
            log.null_output(c"obs_output_can_pause");
            false
        }
    }
}

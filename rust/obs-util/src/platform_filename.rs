//! Safe core for `os_generate_formatted_filename` from
//! `libobs/util/platform-filename.c` (obs-rust/obs-studio#262 slice 5): a
//! recording file name from a format with date, time and video specifiers.
//!
//! The date and time specifiers go through the host C library's
//! `localtime` and `strftime` ([`platform::crt::LocalTime`]), as in C, so
//! names, abbreviations and offsets come out in the same locale and time
//! zone.

use crate::platform::{self, crt::LocalTime};

/// The longest name, in bytes: a longer one is cut to this, or to the last
/// UTF-8 character boundary before it.
pub const MAX_LEN: usize = 255;

/// The `struct obs_video_info` fields the specifiers read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VideoInfo {
    /// `fps_num`.
    pub fps_num: u32,
    /// `fps_den`.
    pub fps_den: u32,
    /// `base_width`.
    pub base_width: u32,
    /// `base_height`.
    pub base_height: u32,
    /// `output_width`.
    pub output_width: u32,
    /// `output_height`.
    pub output_height: u32,
    /// `output_format`, a C `enum video_format` value.
    pub output_format: i32,
}

/// The time a name is made for: `time(0)` and its `localtime`.
#[derive(Clone, Copy)]
pub struct Now {
    /// The Unix time, in seconds (`%s`).
    pub time: i64,
    /// `localtime(&time)`, or `None` when that returned NULL.
    pub local: Option<LocalTime>,
}

impl Now {
    /// `time` and `localtime(&time)`.
    pub fn at(time: i64) -> Self {
        Self {
            time,
            local: LocalTime::of(time),
        }
    }

    /// The current time: `time(0)`, then `localtime`, as the C reads them.
    pub fn current() -> Self {
        Self::at(platform::crt::time_now())
    }
}

/// `get_video_format_name` (`libobs/media-io/video-io.h`): the short name
/// of a C `enum video_format` value, `"None"` for an unknown one.
pub fn video_format_name(_format: i32) -> &'static str {
    todo!("get_video_format_name port")
}

/// `os_generate_formatted_filename(extension, space, format)`, for the
/// time `now` and the video info `video`. A NULL `extension` or `format`
/// is the empty one. `None` is the C's NULL, returned for an empty name.
pub fn generate_formatted_filename(
    _extension: &[u8],
    _space: bool,
    _format: &[u8],
    _now: &Now,
    _video: &VideoInfo,
) -> Option<Vec<u8>> {
    todo!("os_generate_formatted_filename port")
}

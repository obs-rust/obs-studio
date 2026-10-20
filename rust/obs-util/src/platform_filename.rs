//! Safe core for `os_generate_formatted_filename` from
//! `libobs/util/platform-filename.c` (obs-rust/obs-studio#262 slice 5): a
//! recording file name from a format with date, time and video specifiers.
//!
//! The date and time specifiers go through the host C library's
//! `localtime` and `strftime` ([`platform::crt::LocalTime`]), as in C, so
//! names, abbreviations and offsets come out in the same locale and time
//! zone.

use crate::platform::{self, DoubleFormat, crt::LocalTime};

/// The longest name, in bytes: a longer one is cut to this, or to the last
/// UTF-8 character boundary before it.
pub const MAX_LEN: usize = 255;

/// The specifier table, in the C's order: an OBS specifier and the
/// `strftime` conversion it stands for, or a `strftime` conversion passed as
/// it is. The first entry that matches wins.
const SPEC: [(&[u8], &core::ffi::CStr); 23] = [
    (b"%CCYY", c"%Y"),
    (b"%YY", c"%y"),
    (b"%MM", c"%m"),
    (b"%DD", c"%d"),
    (b"%hh", c"%H"),
    (b"%mm", c"%M"),
    (b"%ss", c"%S"),
    (b"%%", c"%%"),
    (b"%a", c"%a"),
    (b"%A", c"%A"),
    (b"%b", c"%b"),
    (b"%B", c"%B"),
    (b"%d", c"%d"),
    (b"%H", c"%H"),
    (b"%I", c"%I"),
    (b"%m", c"%m"),
    (b"%M", c"%M"),
    (b"%p", c"%p"),
    (b"%S", c"%S"),
    (b"%y", c"%y"),
    (b"%Y", c"%Y"),
    (b"%z", c"%z"),
    (b"%Z", c"%Z"),
];

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

    fn strftime(&self, format: &core::ffi::CStr) -> Vec<u8> {
        // The C passes a NULL `struct tm` to strftime then, which is
        // undefined; `localtime` of the current time does not fail.
        self.local.map_or_else(Vec::new, |tm| tm.strftime(format))
    }
}

/// `get_video_format_name` (`libobs/media-io/video-io.h`): the short name
/// of a C `enum video_format` value, `"None"` for an unknown one.
pub fn video_format_name(format: i32) -> &'static str {
    const NAMES: [&str; 26] = [
        "None", "I420", "NV12", "YVYU", "YUY2", "UYVY", "RGBA", "BGRA", "BGRX", "Y800", "I444",
        "BGR3", "I422", "I40A", "I42A", "YUVA", "AYUV", "I010", "P010", "I210", "I412", "YA2L",
        "P216", "P416", "v210", "R10l",
    ];
    usize::try_from(format)
        .ok()
        .and_then(|i| NAMES.get(i))
        .copied()
        .unwrap_or("None")
}

/// `valid_string`: whether `s` has a byte other than a space.
fn valid_string(s: &[u8]) -> bool {
    s.iter().any(|&b| b != b' ')
}

/// `%FPS`: the integer `fps_num` when `fps_den` is 0 or 1, otherwise the
/// quotient with two decimals, from the host C library's `snprintf`.
/// Characterized, not endorsed: `50/2` is `25.00`.
fn fps_text(video: &VideoInfo) -> Vec<u8> {
    if video.fps_den <= 1 {
        video.fps_num.to_string().into_bytes()
    } else {
        let fps = f64::from(video.fps_num) / f64::from(video.fps_den);
        platform::snprintf_double(DoubleFormat::F2, fps)
    }
}

/// The non-table specifier at the start of `rest` and its text, in the C's
/// order.
fn video_or_unix_time(rest: &[u8], now: &Now, video: &VideoInfo) -> Option<(usize, Vec<u8>)> {
    if rest.starts_with(b"%FPS") {
        Some((4, fps_text(video)))
    } else if rest.starts_with(b"%CRES") {
        let text = format!("{}x{}", video.base_width, video.base_height);
        Some((5, text.into_bytes()))
    } else if rest.starts_with(b"%ORES") {
        let text = format!("{}x{}", video.output_width, video.output_height);
        Some((5, text.into_bytes()))
    } else if rest.starts_with(b"%VF") {
        Some((
            3,
            video_format_name(video.output_format).as_bytes().to_vec(),
        ))
    } else if rest.starts_with(b"%s") {
        Some((2, now.time.to_string().into_bytes()))
    } else {
        None
    }
}

/// `os_generate_formatted_filename(extension, space, format)`, for the
/// time `now` and the video info `video`. A NULL `extension` or `format`
/// is the empty one. `None` is the C's NULL, returned for an empty name.
///
/// The format is scanned once, left to right. At each `%` the specifier
/// table is tried in order, then `%FPS`, `%CRES`, `%ORES`, `%VF` and `%s`.
/// A match is replaced by its text and the scan goes on after that text,
/// so a replacement is never scanned again. A `strftime` result that is
/// empty tries the next table entry; one of only spaces is skipped over
/// without being put in (both as in C). A `%` that starts nothing is
/// dropped. With `space` false every space becomes `_`. A non-empty
/// extension is added after a `.`, and the whole name is cut to
/// [`MAX_LEN`] bytes on a UTF-8 character boundary.
pub fn generate_formatted_filename(
    extension: &[u8],
    space: bool,
    format: &[u8],
    now: &Now,
    video: &VideoInfo,
) -> Option<Vec<u8>> {
    let mut sf = format.to_vec();
    let mut pos = 0;

    while pos < sf.len() {
        let mut convert = Vec::new();

        for (spec, conversion) in SPEC {
            if !convert.is_empty() {
                break;
            }
            if sf[pos..].starts_with(spec) {
                convert = now.strftime(conversion);
                if !convert.is_empty() && valid_string(&convert) {
                    sf.splice(pos..pos + spec.len(), convert.iter().copied());
                }
            }
        }

        if convert.is_empty()
            && let Some((len, text)) = video_or_unix_time(&sf[pos..], now, video)
        {
            sf.splice(pos..pos + len, text.iter().copied());
            convert = text;
        }

        if !convert.is_empty() {
            pos += convert.len();
        } else if sf[pos] == b'%' {
            sf.remove(pos);
        } else {
            pos += 1;
        }
    }

    if !space {
        for b in &mut sf {
            if *b == b' ' {
                *b = b'_';
            }
        }
    }

    if !extension.is_empty() {
        sf.push(b'.');
        sf.extend_from_slice(extension);
    }

    if sf.len() > MAX_LEN {
        // Back up to a UTF-8 character boundary so a multibyte character is
        // not split; a character has at most three continuation bytes, so
        // invalid input stops at 252.
        let mut len = MAX_LEN;
        while len > MAX_LEN - 3 && sf[len] & 0xC0 == 0x80 {
            len -= 1;
        }
        sf.truncate(len);
    }

    (!sf.is_empty()).then_some(sf)
}

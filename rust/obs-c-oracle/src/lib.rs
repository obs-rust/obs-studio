//! Test-only: the original libobs C sources compiled with `oracle_`-prefixed
//! symbols, used as the reference in Tier 2 layout tests and Tier 3
//! differential tests (`docs/rust-port/testing-policy.md`). Delete each
//! oracle together with the C source it wraps.
//!
//! Exempt from the port crates' `deny(unsafe_code)` and from the platform
//! boundary Dylint (#297, #323): every item here is a raw `extern "C"`
//! binding to the original C, so `unsafe` and per-OS `cfg` are the crate's
//! purpose. It is a dev-dependency only and never links into libobs.
//!
//! Also compiles the original libobs sources itself: `oracle/platform.c` and
//! `oracle/platform_nix_posix.c` include `util/platform-conversions.c.h` and
//! `util/platform-nix.c.h` unmodified with their exports renamed, so the platform
//! functions the oracle needs come from libobs rather than from hand copies
//! (they used to live in `oracle/test_stubs.c`, `oracle/platform_conv_host.c`,
//! `oracle/config_file_host.c`, `oracle/file_serializer_host.c` and
//! `oracle/obs_data_host.c`).
//! `oracle/platform_host.c` holds the few that cannot be compiled here: the
//! breakpoint pair, which aborts loudly instead of raising the platform trap,
//! and a weak zero-clock fallback. `oracle/platform.c` itself ends with the
//! `obs_get_video_info` and clock fixtures of `platform-filename.c`.

// Fingerprint of the native inputs build.rs feeds to `cc` (oracle/*.c, *.h,
// the libobs/deps sources it compiles, the flag list). Reading it with
// `env!` records a rustc `env-dep`, which the compilation cache folds into
// this crate's artifact key — without it a native-only change can be served
// a stale cached rlib whose bundled objects predate the change.
const _: &str = env!("OBS_ORACLE_INPUT_HASH");

pub mod array_serializer;
pub mod audio_controls;
#[cfg(any(unix, windows))]
pub mod audio_io;
pub mod audio_io_inlines;
pub mod audio_math;
pub mod av1;
pub mod avc;
pub mod base;
pub mod bitstream;
pub mod bmem;
pub mod buffered_file_serializer;
pub mod calldata;
pub mod cf_tokenizer;
pub mod config_file;
pub mod context_names;
pub mod crc32;
pub mod darray;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub mod dbus_inhibit;
pub mod decl;
pub mod deque;
pub mod dstr;
pub mod dstr_libc;
pub mod effect_lookups;
pub mod effect_parser_lifecycle;
pub mod encoder_packet;
pub mod file_serializer;
pub mod format_conversion;
pub mod frame_rate;
pub mod graphics_math;
pub mod hevc;
pub mod hotkey_bindings;
pub mod hotkey_name_map;
pub mod hotkey_pair;
pub mod hotkey_press;
pub mod hotkey_translations;
pub mod lexer;
pub mod missing_files;
pub mod nal;
pub mod nix_hotkey_state;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub mod nix_inhibit;
pub mod obs_nvenc;
pub mod obs_properties;
pub mod output_caption_text;
pub mod output_delay;
pub mod output_interleave;
pub mod output_protocols;
pub mod path_extension;
pub mod pipe;
#[cfg(target_os = "macos")]
pub mod platform_cocoa;
pub mod platform_conversions;
pub mod platform_file;
pub mod platform_filename;
pub mod platform_mkdirs;
#[cfg(unix)]
pub mod platform_nix;
pub mod platform_number;
pub mod platform_time;
#[cfg(all(windows, target_env = "msvc"))]
pub mod platform_windows;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub mod portal_inhibit;
pub mod portal_shortcut_ids;
pub mod portal_shortcut_sets;
pub mod proc;
pub mod profiler;
pub mod profiler_io;
pub mod profiler_snapshot;
pub mod pulse_format;
pub mod pulse_stream_flags;
pub mod scene_audio_actions;
pub mod scene_blend;
pub mod scene_bounds;
pub mod scene_bounds_place;
pub mod scene_coords;
pub mod scene_group_transform;
pub mod shader_param_names;
pub mod shader_sampler_convert;
pub mod signal;
pub mod task;
pub mod text_lookup;
pub mod texture_render;
pub mod threading;
pub mod utf8;
pub mod util_uint64;
pub mod vec2;
pub mod video_fourcc;
pub mod video_frame;
pub mod video_io;
pub mod video_io_inlines;
pub mod video_matrices;
pub mod video_plan;

/// Build directory of the original C archive, for symbol-isolation checks.
pub const ORACLE_ARCHIVE_DIR: &str = env!("OUT_DIR");

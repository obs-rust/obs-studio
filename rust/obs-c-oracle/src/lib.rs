//! Test-only: the original libobs C sources compiled with `oracle_`-prefixed
//! symbols, used as the reference in Tier 2 layout tests and Tier 3
//! differential tests (`docs/rust-port/testing-policy.md`). Delete each
//! oracle together with the C source it wraps.
//!
//! Also provides test-only stand-ins for libobs base/platform symbols
//! (`os_breakpoint`, `os_oom`, `os_fread_utf8` from `oracle/test_stubs.c`;
//! `os_fopen` and friends from `oracle/file_serializer_host.c`; the platform.c
//! string conversions from `oracle/platform_conv_host.c`; `blog`/`bcrash`
//! from `util/base-variadic.c`), so any test binary using the oracle resolves
//! them.

pub mod bmem;
pub mod dstr;
pub mod lexer;
pub mod text_lookup;
pub mod utf8;

pub mod path_extension {
    use core::ffi::c_char;

    unsafe extern "C" {
        pub fn oracle_os_get_path_extension(path: *const c_char) -> *const c_char;
    }
}

pub mod crc32 {
    unsafe extern "C" {
        pub fn oracle_calc_crc32(crc: u32, buf: *const core::ffi::c_void, size: usize) -> u32;
    }
}

pub mod vec2 {
    use core::ffi::c_int;

    /// Independent declaration of `struct vec2`. Intentionally not shared
    /// with `obs-graphics`.
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct OracleVec2 {
        pub x: f32,
        pub y: f32,
    }

    unsafe extern "C" {
        pub fn oracle_vec2_abs(dst: *mut OracleVec2, v: *const OracleVec2);
        pub fn oracle_vec2_floor(dst: *mut OracleVec2, v: *const OracleVec2);
        pub fn oracle_vec2_ceil(dst: *mut OracleVec2, v: *const OracleVec2);
        pub fn oracle_vec2_close(
            v1: *const OracleVec2,
            v2: *const OracleVec2,
            epsilon: f32,
        ) -> c_int;
        pub fn oracle_vec2_norm(dst: *mut OracleVec2, v: *const OracleVec2);

        pub fn oracle_vec2_size() -> usize;
        pub fn oracle_vec2_align() -> usize;
        pub fn oracle_vec2_offset_x() -> usize;
        pub fn oracle_vec2_offset_y() -> usize;
        pub fn oracle_vec2_offset_ptr() -> usize;
    }
}

pub mod cf_tokenizer {
    //! `util/cf-tokenizer.c`, running on the lexer oracle.
    use core::ffi::{c_char, c_int};

    use super::darray::OracleDarray;
    use super::lexer::{OracleLexer, OracleStrref};

    /// Independent declaration of `struct cf_token`.
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct OracleCfToken {
        pub lex: *const OracleCfLexer,
        pub str: OracleStrref,
        pub unmerged_str: OracleStrref,
        pub kind: c_int,
    }

    /// Independent declaration of `struct cf_lexer`.
    #[repr(C)]
    #[derive(Debug)]
    pub struct OracleCfLexer {
        pub file: *mut c_char,
        pub base_lexer: OracleLexer,
        pub reformatted: *mut c_char,
        pub write_offset: *mut c_char,
        pub tokens: OracleDarray,
        pub unexpected_eof: bool,
    }

    unsafe extern "C" {
        pub fn oracle_cf_literal_to_str(literal: *const c_char, count: usize) -> *mut c_char;
        pub fn oracle_cf_lexer_init(lex: *mut OracleCfLexer);
        pub fn oracle_cf_lexer_free(lex: *mut OracleCfLexer);
        pub fn oracle_cf_lexer_lex(
            lex: *mut OracleCfLexer,
            str: *const c_char,
            file: *const c_char,
        ) -> bool;

        pub fn oracle_cf_token_size() -> usize;
        pub fn oracle_cf_token_align() -> usize;
        pub fn oracle_cf_token_offset_lex() -> usize;
        pub fn oracle_cf_token_offset_str() -> usize;
        pub fn oracle_cf_token_offset_unmerged_str() -> usize;
        pub fn oracle_cf_token_offset_type() -> usize;
        pub fn oracle_cf_token_type_size() -> usize;
        pub fn oracle_cf_lexer_size() -> usize;
        pub fn oracle_cf_lexer_align() -> usize;
        pub fn oracle_cf_lexer_offset_file() -> usize;
        pub fn oracle_cf_lexer_offset_base_lexer() -> usize;
        pub fn oracle_cf_lexer_offset_reformatted() -> usize;
        pub fn oracle_cf_lexer_offset_write_offset() -> usize;
        pub fn oracle_cf_lexer_offset_tokens() -> usize;
        pub fn oracle_cf_lexer_offset_unexpected_eof() -> usize;
    }
}

pub mod darray {
    use core::ffi::c_void;

    /// Independent declaration of `struct darray`. Intentionally not shared
    /// with `obs-util`.
    #[repr(C)]
    #[derive(Debug)]
    pub struct OracleDarray {
        pub array: *mut c_void,
        pub num: usize,
        pub capacity: usize,
    }

    unsafe extern "C" {
        pub fn oracle_darray_free(da: *mut OracleDarray);
        pub fn oracle_darray_reserve(es: usize, da: *mut OracleDarray, capacity: usize);
        pub fn oracle_darray_ensure_capacity(es: usize, da: *mut OracleDarray, new_size: usize);
        pub fn oracle_darray_resize(es: usize, da: *mut OracleDarray, size: usize);
        pub fn oracle_darray_clear(da: *mut OracleDarray);
        pub fn oracle_darray_push_back_array(
            es: usize,
            da: *mut OracleDarray,
            array: *const c_void,
            num: usize,
        ) -> usize;
        pub fn oracle_darray_erase(es: usize, da: *mut OracleDarray, idx: usize);
        pub fn oracle_darray_pop_back(es: usize, da: *mut OracleDarray);

        pub fn oracle_darray_size() -> usize;
        pub fn oracle_darray_align() -> usize;
        pub fn oracle_darray_offset_array() -> usize;
        pub fn oracle_darray_offset_num() -> usize;
        pub fn oracle_darray_offset_capacity() -> usize;
    }
}

pub mod array_serializer {
    use core::ffi::{c_int, c_void};

    /// Independent declaration of `struct serializer`.
    #[repr(C)]
    pub struct OracleSerializer {
        pub data: *mut c_void,
        pub read: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> usize>,
        pub write: Option<unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> usize>,
        pub seek: Option<unsafe extern "C" fn(*mut c_void, i64, c_int) -> i64>,
        pub get_pos: Option<unsafe extern "C" fn(*mut c_void) -> i64>,
    }

    /// Independent declaration of `struct array_output_data`.
    #[repr(C)]
    pub struct OracleArrayOutputData {
        pub bytes: super::darray::OracleDarray,
        pub cur_pos: usize,
    }

    unsafe extern "C" {
        pub fn oracle_array_output_serializer_init(
            s: *mut OracleSerializer,
            data: *mut OracleArrayOutputData,
        );
        pub fn oracle_array_output_serializer_free(data: *mut OracleArrayOutputData);
        pub fn oracle_array_output_serializer_reset(data: *mut OracleArrayOutputData);

        pub fn oracle_serializer_size() -> usize;
        pub fn oracle_serializer_align() -> usize;
        pub fn oracle_serializer_offset_data() -> usize;
        pub fn oracle_serializer_offset_read() -> usize;
        pub fn oracle_serializer_offset_write() -> usize;
        pub fn oracle_serializer_offset_seek() -> usize;
        pub fn oracle_serializer_offset_get_pos() -> usize;

        pub fn oracle_array_output_data_size() -> usize;
        pub fn oracle_array_output_data_align() -> usize;
        pub fn oracle_array_output_data_offset_bytes() -> usize;
        pub fn oracle_array_output_data_offset_cur_pos() -> usize;
    }
}

pub mod base {
    use core::ffi::c_void;

    unsafe extern "C" {
        pub fn oracle_base_get_log_handler(handler: *mut *mut c_void, param: *mut *mut c_void);
        pub fn oracle_base_set_log_handler(handler: *mut c_void, param: *mut c_void);
        pub fn oracle_base_set_crash_handler(handler: *mut c_void, param: *mut c_void);
    }
}

pub mod bitstream {
    use core::ffi::c_int;

    /// Independent declaration of `struct bitstream_reader` for calling the
    /// oracle. Intentionally not shared with `obs-util`.
    #[repr(C)]
    #[allow(non_snake_case)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OracleReader {
        pub pos: u8,
        pub subPos: u8,
        pub buf: *mut u8,
        pub len: usize,
    }

    unsafe extern "C" {
        pub fn oracle_bitstream_reader_init(r: *mut OracleReader, data: *mut u8, len: usize);
        pub fn oracle_bitstream_reader_read_bits(r: *mut OracleReader, bits: c_int) -> u8;
        pub fn oracle_bitstream_reader_r8(r: *mut OracleReader) -> u8;
        pub fn oracle_bitstream_reader_r16(r: *mut OracleReader) -> u16;

        pub fn oracle_bitstream_reader_size() -> usize;
        pub fn oracle_bitstream_reader_align() -> usize;
        pub fn oracle_bitstream_reader_offset_pos() -> usize;
        pub fn oracle_bitstream_reader_offset_subPos() -> usize;
        pub fn oracle_bitstream_reader_offset_buf() -> usize;
        pub fn oracle_bitstream_reader_offset_len() -> usize;
    }
}

pub mod graphics_math {
    //! The graphics math cluster (vec3, vec4, matrix3, matrix4, quat, plane,
    //! bounds, axisang, math-extra). The C files call each other, so all of
    //! them are compiled; only the functions a port needs are declared here.

    use core::ffi::c_int;

    /// Independent declaration of `struct vec3`: four floats in a union
    /// with `__m128`, so 16 bytes aligned to 16.
    #[repr(C, align(16))]
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct OracleVec3 {
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub w: f32,
    }

    /// Independent declaration of `struct vec4`.
    #[repr(C, align(16))]
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct OracleVec4 {
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub w: f32,
    }

    /// Independent declaration of `struct matrix4`.
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct OracleMatrix4 {
        pub x: OracleVec4,
        pub y: OracleVec4,
        pub z: OracleVec4,
        pub t: OracleVec4,
    }

    /// Independent declaration of `struct plane`.
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct OraclePlane {
        pub dir: OracleVec3,
        pub dist: f32,
    }

    /// Independent declaration of `struct matrix3`.
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct OracleMatrix3 {
        pub x: OracleVec3,
        pub y: OracleVec3,
        pub z: OracleVec3,
        pub t: OracleVec3,
    }

    /// Independent declaration of `struct axisang` (no `__m128` member).
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct OracleAxisAng {
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub w: f32,
    }

    /// Independent declaration of `struct quat`.
    #[repr(C, align(16))]
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct OracleQuat {
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub w: f32,
    }

    unsafe extern "C" {
        pub fn oracle_axisang_from_quat(dst: *mut OracleAxisAng, q: *const OracleQuat);

        pub fn oracle_axisang_size() -> usize;
        pub fn oracle_axisang_align() -> usize;
        pub fn oracle_axisang_offset_x() -> usize;
        pub fn oracle_axisang_offset_y() -> usize;
        pub fn oracle_axisang_offset_z() -> usize;
        pub fn oracle_axisang_offset_w() -> usize;
        pub fn oracle_axisang_offset_ptr() -> usize;
        pub fn oracle_quat_size() -> usize;
        pub fn oracle_quat_align() -> usize;
        pub fn oracle_quat_offset_x() -> usize;
        pub fn oracle_quat_offset_y() -> usize;
        pub fn oracle_quat_offset_z() -> usize;
        pub fn oracle_quat_offset_w() -> usize;
        pub fn oracle_quat_offset_m() -> usize;
    }

    unsafe extern "C" {
        pub fn oracle_vec3_from_vec4(dst: *mut OracleVec3, v: *const OracleVec4);
        pub fn oracle_vec3_plane_dist(v: *const OracleVec3, p: *const OraclePlane) -> f32;
        pub fn oracle_vec3_rotate(
            dst: *mut OracleVec3,
            v: *const OracleVec3,
            m: *const OracleMatrix3,
        );
        pub fn oracle_vec3_transform(
            dst: *mut OracleVec3,
            v: *const OracleVec3,
            m: *const OracleMatrix4,
        );
        pub fn oracle_vec3_transform3x4(
            dst: *mut OracleVec3,
            v: *const OracleVec3,
            m: *const OracleMatrix3,
        );
        pub fn oracle_vec3_mirror(
            dst: *mut OracleVec3,
            v: *const OracleVec3,
            p: *const OraclePlane,
        );
        pub fn oracle_vec3_mirrorv(
            dst: *mut OracleVec3,
            v: *const OracleVec3,
            vec: *const OracleVec3,
        );
        pub fn oracle_vec3_rand(dst: *mut OracleVec3, positive_only: c_int);

        pub fn oracle_vec3_offset_x() -> usize;
        pub fn oracle_vec3_offset_y() -> usize;
        pub fn oracle_vec3_offset_z() -> usize;
        pub fn oracle_vec3_offset_w() -> usize;
        pub fn oracle_vec3_offset_ptr() -> usize;
        pub fn oracle_vec3_offset_m() -> usize;
        pub fn oracle_plane_size() -> usize;
        pub fn oracle_plane_align() -> usize;
        pub fn oracle_plane_offset_dir() -> usize;
        pub fn oracle_plane_offset_dist() -> usize;
        pub fn oracle_matrix3_size() -> usize;
        pub fn oracle_matrix3_align() -> usize;
        pub fn oracle_matrix3_offset_x() -> usize;
        pub fn oracle_matrix3_offset_y() -> usize;
        pub fn oracle_matrix3_offset_z() -> usize;
        pub fn oracle_matrix3_offset_t() -> usize;
    }

    unsafe extern "C" {
        pub fn oracle_vec4_from_vec3(dst: *mut OracleVec4, v: *const OracleVec3);
        pub fn oracle_vec4_transform(
            dst: *mut OracleVec4,
            v: *const OracleVec4,
            m: *const OracleMatrix4,
        );

        pub fn oracle_vec3_size() -> usize;
        pub fn oracle_vec3_align() -> usize;
        pub fn oracle_vec4_size() -> usize;
        pub fn oracle_vec4_align() -> usize;
        pub fn oracle_vec4_offset_x() -> usize;
        pub fn oracle_vec4_offset_y() -> usize;
        pub fn oracle_vec4_offset_z() -> usize;
        pub fn oracle_vec4_offset_w() -> usize;
        pub fn oracle_vec4_offset_ptr() -> usize;
        pub fn oracle_vec4_offset_m() -> usize;
        pub fn oracle_matrix4_size() -> usize;
        pub fn oracle_matrix4_align() -> usize;
        pub fn oracle_matrix4_offset_x() -> usize;
        pub fn oracle_matrix4_offset_y() -> usize;
        pub fn oracle_matrix4_offset_z() -> usize;
        pub fn oracle_matrix4_offset_t() -> usize;
    }
}

pub mod task {
    use core::ffi::c_void;

    /// `struct os_task_queue`, opaque in `util/task.h`.
    pub enum OracleTaskQueue {}

    /// `os_task_t`.
    pub type OracleOsTask = Option<unsafe extern "C" fn(*mut c_void)>;

    unsafe extern "C" {
        pub fn oracle_os_task_queue_create() -> *mut OracleTaskQueue;
        pub fn oracle_os_task_queue_queue_task(
            tq: *mut OracleTaskQueue,
            task: OracleOsTask,
            param: *mut c_void,
        ) -> bool;
        pub fn oracle_os_task_queue_destroy(tq: *mut OracleTaskQueue);
        pub fn oracle_os_task_queue_wait(tq: *mut OracleTaskQueue) -> bool;
        pub fn oracle_os_task_queue_inside(tq: *mut OracleTaskQueue) -> bool;
    }
}

pub mod file_serializer {
    use core::ffi::c_char;

    pub use super::array_serializer::OracleSerializer;

    unsafe extern "C" {
        pub fn oracle_file_input_serializer_init(
            s: *mut OracleSerializer,
            path: *const c_char,
        ) -> bool;
        pub fn oracle_file_input_serializer_free(s: *mut OracleSerializer);
        pub fn oracle_file_output_serializer_init(
            s: *mut OracleSerializer,
            path: *const c_char,
        ) -> bool;
        pub fn oracle_file_output_serializer_init_safe(
            s: *mut OracleSerializer,
            path: *const c_char,
            temp_ext: *const c_char,
        ) -> bool;
        pub fn oracle_file_output_serializer_free(s: *mut OracleSerializer);
    }
}

pub mod nal {
    //! `libobs/obs-nal.c`.

    unsafe extern "C" {
        pub fn oracle_obs_nal_find_startcode(p: *const u8, end: *const u8) -> *const u8;
    }
}

pub mod video_fourcc {
    use core::ffi::c_int;

    unsafe extern "C" {
        /// Returns `enum video_format` as the C `int` it is passed as.
        pub fn oracle_video_format_from_fourcc(fourcc: u32) -> c_int;

        pub fn oracle_video_format_size() -> usize;
        pub fn oracle_video_format_align() -> usize;
        pub fn oracle_video_format_count() -> usize;
        pub fn oracle_video_format_value(i: usize) -> c_int;
    }
}

pub mod encoder_packet {
    //! `struct encoder_packet` from `libobs/obs-encoder.h` and its C layout.
    use core::ffi::{c_int, c_void};

    /// Independent declaration of `struct encoder_packet`.
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct OracleEncoderPacket {
        pub data: *mut u8,
        pub size: usize,
        pub pts: i64,
        pub dts: i64,
        pub timebase_num: i32,
        pub timebase_den: i32,
        pub type_: c_int,
        pub keyframe: bool,
        pub dts_usec: i64,
        pub sys_dts_usec: i64,
        pub priority: c_int,
        pub drop_priority: c_int,
        pub track_idx: usize,
        pub encoder: *mut c_void,
    }

    unsafe extern "C" {
        pub fn oracle_encoder_packet_size() -> usize;
        pub fn oracle_encoder_packet_align() -> usize;
        pub fn oracle_sizeof_long() -> usize;
        pub fn oracle_encoder_packet_offset_data() -> usize;
        pub fn oracle_encoder_packet_offset_size() -> usize;
        pub fn oracle_encoder_packet_offset_pts() -> usize;
        pub fn oracle_encoder_packet_offset_dts() -> usize;
        pub fn oracle_encoder_packet_offset_timebase_num() -> usize;
        pub fn oracle_encoder_packet_offset_timebase_den() -> usize;
        pub fn oracle_encoder_packet_offset_type() -> usize;
        pub fn oracle_encoder_packet_offset_keyframe() -> usize;
        pub fn oracle_encoder_packet_offset_dts_usec() -> usize;
        pub fn oracle_encoder_packet_offset_sys_dts_usec() -> usize;
        pub fn oracle_encoder_packet_offset_priority() -> usize;
        pub fn oracle_encoder_packet_offset_drop_priority() -> usize;
        pub fn oracle_encoder_packet_offset_track_idx() -> usize;
        pub fn oracle_encoder_packet_offset_encoder() -> usize;
    }
}

pub mod hevc {
    //! `libobs/obs-hevc.c`, calling the oracle copies of obs-nal.c and
    //! array-serializer.c.
    use core::ffi::c_int;

    pub use super::encoder_packet::OracleEncoderPacket;

    unsafe extern "C" {
        pub fn oracle_obs_hevc_keyframe(data: *const u8, size: usize) -> bool;
        pub fn oracle_obs_parse_hevc_packet(
            hevc_packet: *mut OracleEncoderPacket,
            src: *const OracleEncoderPacket,
        );
        pub fn oracle_obs_parse_hevc_packet_priority(packet: *const OracleEncoderPacket) -> c_int;
        #[allow(clippy::too_many_arguments)]
        pub fn oracle_obs_extract_hevc_headers(
            packet: *const u8,
            size: usize,
            new_packet_data: *mut *mut u8,
            new_packet_size: *mut usize,
            header_data: *mut *mut u8,
            header_size: *mut usize,
            sei_data: *mut *mut u8,
            sei_size: *mut usize,
        );
    }
}

pub mod profiler_snapshot {
    //! `util/profiler-snapshot.c`: the snapshot accessors and free.
    use core::ffi::{c_char, c_void};

    /// Independent declaration of `DARRAY(T)`.
    #[repr(C)]
    #[derive(Debug)]
    pub struct OracleDarray<T> {
        pub array: *mut T,
        pub num: usize,
        pub capacity: usize,
    }

    /// Independent declaration of `struct profiler_time_entry`.
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct OracleTimeEntry {
        pub time_delta: u64,
        pub count: u64,
    }

    /// Independent declaration of `struct profiler_snapshot_entry`.
    #[repr(C)]
    #[derive(Debug)]
    pub struct OracleSnapshotEntry {
        pub name: *const c_char,
        pub times: OracleDarray<OracleTimeEntry>,
        pub min_time: u64,
        pub max_time: u64,
        pub overall_count: u64,
        pub times_between_calls: OracleDarray<OracleTimeEntry>,
        pub expected_time_between_calls: u64,
        pub min_time_between_calls: u64,
        pub max_time_between_calls: u64,
        pub overall_between_calls_count: u64,
        pub children: OracleDarray<OracleSnapshotEntry>,
    }

    /// Independent declaration of `struct profiler_snapshot`.
    #[repr(C)]
    #[derive(Debug)]
    pub struct OracleSnapshot {
        pub roots: OracleDarray<OracleSnapshotEntry>,
    }

    pub type OracleEnumFunc =
        Option<unsafe extern "C" fn(context: *mut c_void, entry: *mut OracleSnapshotEntry) -> bool>;
    pub type OracleFilterFunc = Option<
        unsafe extern "C" fn(data: *mut c_void, name: *const c_char, remove: *mut bool) -> bool,
    >;

    unsafe extern "C" {
        pub fn oracle_profile_snapshot_free(snap: *mut OracleSnapshot);
        pub fn oracle_profiler_snapshot_num_roots(snap: *mut OracleSnapshot) -> usize;
        pub fn oracle_profiler_snapshot_enumerate_roots(
            snap: *mut OracleSnapshot,
            func: OracleEnumFunc,
            context: *mut c_void,
        );
        pub fn oracle_profiler_snapshot_filter_roots(
            snap: *mut OracleSnapshot,
            func: OracleFilterFunc,
            data: *mut c_void,
        );
        pub fn oracle_profiler_snapshot_num_children(entry: *mut OracleSnapshotEntry) -> usize;
        pub fn oracle_profiler_snapshot_enumerate_children(
            entry: *mut OracleSnapshotEntry,
            func: OracleEnumFunc,
            context: *mut c_void,
        );
        pub fn oracle_profiler_snapshot_entry_name(
            entry: *mut OracleSnapshotEntry,
        ) -> *const c_char;
        pub fn oracle_profiler_snapshot_entry_times(
            entry: *mut OracleSnapshotEntry,
        ) -> *mut OracleDarray<OracleTimeEntry>;
        pub fn oracle_profiler_snapshot_entry_overall_count(entry: *mut OracleSnapshotEntry)
        -> u64;
        pub fn oracle_profiler_snapshot_entry_min_time(entry: *mut OracleSnapshotEntry) -> u64;
        pub fn oracle_profiler_snapshot_entry_max_time(entry: *mut OracleSnapshotEntry) -> u64;
        pub fn oracle_profiler_snapshot_entry_times_between_calls(
            entry: *mut OracleSnapshotEntry,
        ) -> *mut OracleDarray<OracleTimeEntry>;
        pub fn oracle_profiler_snapshot_entry_expected_time_between_calls(
            entry: *mut OracleSnapshotEntry,
        ) -> u64;
        pub fn oracle_profiler_snapshot_entry_min_time_between_calls(
            entry: *mut OracleSnapshotEntry,
        ) -> u64;
        pub fn oracle_profiler_snapshot_entry_max_time_between_calls(
            entry: *mut OracleSnapshotEntry,
        ) -> u64;
        pub fn oracle_profiler_snapshot_entry_overall_between_calls_count(
            entry: *mut OracleSnapshotEntry,
        ) -> u64;

        pub fn oracle_profiler_snapshot_size() -> usize;
        pub fn oracle_profiler_snapshot_align() -> usize;
        pub fn oracle_profiler_snapshot_offset_roots() -> usize;
        pub fn oracle_profiler_snapshot_entry_size() -> usize;
        pub fn oracle_profiler_snapshot_entry_align() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_name() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_times() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_min_time() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_max_time() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_overall_count() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_times_between_calls() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_expected_time_between_calls() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_min_time_between_calls() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_max_time_between_calls() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_overall_between_calls_count() -> usize;
        pub fn oracle_profiler_snapshot_entry_offset_children() -> usize;
        pub fn oracle_profiler_time_entry_size() -> usize;
        pub fn oracle_profiler_time_entry_align() -> usize;
        pub fn oracle_profiler_time_entry_offset_time_delta() -> usize;
        pub fn oracle_profiler_time_entry_offset_count() -> usize;
    }
}

pub mod av1 {
    //! `libobs/obs-av1.c`.

    unsafe extern "C" {
        pub fn oracle_obs_av1_keyframe(data: *const u8, size: usize) -> bool;
        pub fn oracle_obs_extract_av1_headers(
            packet: *const u8,
            size: usize,
            new_packet_data: *mut *mut u8,
            new_packet_size: *mut usize,
            header_data: *mut *mut u8,
            header_size: *mut usize,
        );
        pub fn oracle_metadata_obu_itu_t35(
            itut_t35_buffer: *const u8,
            itut_bufsize: usize,
            out_buffer: *mut *mut u8,
            outbuf_size: *mut usize,
        );
        pub fn oracle_metadata_obu(
            source_buffer: *const u8,
            source_bufsize: usize,
            out_buffer: *mut *mut u8,
            outbuf_size: *mut usize,
            metadata_type: u8,
        );
    }
}

pub mod pipe {
    use core::ffi::c_char;
    #[cfg(unix)]
    use core::ffi::c_int;

    /// Opaque `struct os_process_args`. Only pointers cross the boundary.
    pub enum OracleArgs {}

    /// Opaque `struct os_process_pipe`. Only pointers cross the boundary.
    pub enum OraclePipe {}

    unsafe extern "C" {
        pub fn oracle_os_process_args_create(executable: *const c_char) -> *mut OracleArgs;
        pub fn oracle_os_process_args_add_arg(args: *mut OracleArgs, arg: *const c_char);
        pub fn oracle_os_process_args_add_argf(args: *mut OracleArgs, format: *const c_char, ...);
        pub fn oracle_os_process_args_get_argc(args: *const OracleArgs) -> usize;
        pub fn oracle_os_process_args_get_argv(args: *const OracleArgs) -> *mut *mut c_char;
        pub fn oracle_os_process_args_destroy(args: *mut OracleArgs);

        #[cfg(unix)]
        pub fn oracle_os_process_pipe_create(
            cmd_line: *const c_char,
            type_: *const c_char,
        ) -> *mut OraclePipe;
        #[cfg(unix)]
        pub fn oracle_os_process_pipe_create2(
            args: *const OracleArgs,
            type_: *const c_char,
        ) -> *mut OraclePipe;
        #[cfg(unix)]
        pub fn oracle_os_process_pipe_destroy(pp: *mut OraclePipe) -> c_int;
        #[cfg(unix)]
        pub fn oracle_os_process_pipe_read(pp: *mut OraclePipe, data: *mut u8, len: usize)
        -> usize;
        #[cfg(unix)]
        pub fn oracle_os_process_pipe_read_err(
            pp: *mut OraclePipe,
            data: *mut u8,
            len: usize,
        ) -> usize;
        #[cfg(unix)]
        pub fn oracle_os_process_pipe_write(
            pp: *mut OraclePipe,
            data: *const u8,
            len: usize,
        ) -> usize;
    }
}

pub mod video_matrices {
    use core::ffi::c_int;

    unsafe extern "C" {
        /// `color_space`, `range` and `format` are the C enums as the `int`s
        /// they are passed as. `range_min` and `range_max` may be null.
        pub fn oracle_video_format_get_parameters(
            color_space: c_int,
            range: c_int,
            matrix: *mut f32,
            range_min: *mut f32,
            range_max: *mut f32,
        ) -> bool;
        pub fn oracle_video_format_get_parameters_for_format(
            color_space: c_int,
            range: c_int,
            format: c_int,
            matrix: *mut f32,
            range_min: *mut f32,
            range_max: *mut f32,
        ) -> bool;

        pub fn oracle_video_colorspace_size() -> usize;
        pub fn oracle_video_colorspace_align() -> usize;
        pub fn oracle_video_colorspace_count() -> usize;
        pub fn oracle_video_colorspace_value(i: usize) -> c_int;

        pub fn oracle_video_range_type_size() -> usize;
        pub fn oracle_video_range_type_align() -> usize;
        pub fn oracle_video_range_type_count() -> usize;
        pub fn oracle_video_range_type_value(i: usize) -> c_int;
    }
}

pub mod buffered_file_serializer {
    use core::ffi::c_char;

    pub use super::array_serializer::OracleSerializer;

    unsafe extern "C" {
        pub fn oracle_buffered_file_serializer_init_defaults(
            s: *mut OracleSerializer,
            path: *const c_char,
        ) -> bool;
        pub fn oracle_buffered_file_serializer_init(
            s: *mut OracleSerializer,
            path: *const c_char,
            max_bufsize: usize,
            chunk_size: usize,
        ) -> bool;
        pub fn oracle_buffered_file_serializer_free(s: *mut OracleSerializer);
    }
}

pub mod config_file {
    use core::ffi::{c_char, c_int};

    /// `struct config_data` is opaque; only its address matters.
    #[repr(C)]
    pub struct OracleConfig {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn oracle_config_create(file: *const c_char) -> *mut OracleConfig;
        pub fn oracle_config_open(
            config: *mut *mut OracleConfig,
            file: *const c_char,
            open_type: c_int,
        ) -> c_int;
        pub fn oracle_config_open_string(
            config: *mut *mut OracleConfig,
            str_: *const c_char,
        ) -> c_int;
        pub fn oracle_config_open_defaults(config: *mut OracleConfig, file: *const c_char)
        -> c_int;
        pub fn oracle_config_save(config: *mut OracleConfig) -> c_int;
        pub fn oracle_config_save_safe(
            config: *mut OracleConfig,
            temp_ext: *const c_char,
            backup_ext: *const c_char,
        ) -> c_int;
        pub fn oracle_config_close(config: *mut OracleConfig);
        pub fn oracle_config_num_sections(config: *mut OracleConfig) -> usize;
        pub fn oracle_config_get_section(config: *mut OracleConfig, idx: usize) -> *const c_char;
        pub fn oracle_config_get_string(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> *const c_char;
        pub fn oracle_config_get_int(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> i64;
        pub fn oracle_config_get_uint(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> u64;
        pub fn oracle_config_get_bool(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> bool;
        pub fn oracle_config_get_double(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> f64;
        pub fn oracle_config_remove_value(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> bool;
        pub fn oracle_config_set_string(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
            value: *const c_char,
        );
        pub fn oracle_config_set_int(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
            value: i64,
        );
        pub fn oracle_config_set_uint(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
            value: u64,
        );
        pub fn oracle_config_set_bool(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
            value: bool,
        );
        pub fn oracle_config_set_double(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
            value: f64,
        );
        pub fn oracle_config_set_default_string(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
            value: *const c_char,
        );
        pub fn oracle_config_set_default_int(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
            value: i64,
        );
        pub fn oracle_config_set_default_uint(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
            value: u64,
        );
        pub fn oracle_config_set_default_bool(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
            value: bool,
        );
        pub fn oracle_config_set_default_double(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
            value: f64,
        );
        pub fn oracle_config_get_default_string(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> *const c_char;
        pub fn oracle_config_get_default_int(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> i64;
        pub fn oracle_config_get_default_uint(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> u64;
        pub fn oracle_config_get_default_bool(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> bool;
        pub fn oracle_config_get_default_double(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> f64;
        pub fn oracle_config_has_user_value(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> bool;
        pub fn oracle_config_has_default_value(
            config: *mut OracleConfig,
            section: *const c_char,
            name: *const c_char,
        ) -> bool;
    }
}

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

//! Tier 1: `obs_source::conversion_plan`, the async video GPU conversion
//! plan of `obs-source.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_async_conversion_plan.c` 1:1,
//! under the same case names, in the same order, with the same inputs and
//! expectations: its two generated tables (336 rows of format x range x
//! transfer, and the plane sizes of the 21 convert types at its 14 sizes)
//! are copied below unchanged.
//!
//! The `edge_*` cases are new: the convert type values; proptests over any
//! `i32` format and transfer and any `u32` size (values outside the enum are
//! no conversion, the transfer counts modulo 256, and the sizes against a
//! model in `u64` arithmetic reduced modulo 2^32); every plan's planes and
//! technique against its convert type; and every technique against
//! `libobs/data/format_conversion.effect`.

use std::ffi::CStr;

use obs_source::conversion_plan::{
    ConvertType, PLANES, Plan, Plane, convert_type, gs_color_format as GS, half, linear, plan,
    planes, rgb_limited_format, technique, video_format as F,
};
use proptest::prelude::*;

use ConvertType as CT;

/// `enum video_trc` as the C passes it, an `int`.
const TRC_DEFAULT: i32 = 0;
const TRC_SRGB: i32 = 1;
const TRC_PQ: i32 = 2;
const TRC_HLG: i32 = 3;

/// The formats of the C test: every `enum video_format`, two values outside.
const FORMATS: [i32; 28] = [
    F::NONE,
    F::I420,
    F::NV12,
    F::YVYU,
    F::YUY2,
    F::UYVY,
    F::RGBA,
    F::BGRA,
    F::BGRX,
    F::Y800,
    F::I444,
    F::BGR3,
    F::I422,
    F::I40A,
    F::I42A,
    F::YUVA,
    F::AYUV,
    F::I010,
    F::P010,
    F::I210,
    F::I412,
    F::YA2L,
    F::P216,
    F::P416,
    F::V210,
    F::R10L,
    F::R10L + 1,
    -1,
];

/// The transfers of the C test: every `enum video_trc`, 4, and PQ + 256.
const TRCS: [i32; 6] = [TRC_DEFAULT, TRC_SRGB, TRC_PQ, TRC_HLG, 4, 256 + TRC_PQ];

/// The sizes of the C test.
const SIZES: [(u32, u32); 14] = [
    (0, 0),
    (1, 1),
    (2, 2),
    (3, 3),
    (1281, 721),
    (1920, 1080),
    (0x5555_5555, 0x5555_5555),
    (0x5555_5556, 7),
    (u32::MAX - 5, u32::MAX - 5),
    (u32::MAX - 4, u32::MAX - 4),
    (u32::MAX - 1, u32::MAX - 1),
    (u32::MAX, u32::MAX),
    (u32::MAX, 1),
    (1, u32::MAX),
];

/// One row of the C test's `conversion_table`.
#[derive(Debug, Clone, Copy)]
struct Row {
    format: i32,
    full_range: bool,
    trc: i32,
    convert_type: ConvertType,
    channel_count: usize,
    plane_format: [i32; PLANES],
    technique: Option<&'static CStr>,
    linear: bool,
}

#[allow(clippy::too_many_arguments)]
const fn row(
    format: i32,
    full_range: bool,
    trc: i32,
    convert_type: ConvertType,
    channel_count: usize,
    plane_format: [i32; PLANES],
    technique: Option<&'static CStr>,
    linear: bool,
) -> Row {
    Row {
        format,
        full_range,
        trc,
        convert_type,
        channel_count,
        plane_format,
        technique,
        linear,
    }
}

/// `conversion_table` of the C test: formats x ranges x transfers, in order.
#[rustfmt::skip]
static CONVERSION_TABLE: [Row; 336] = [
    row(F::NONE, false, TRC_DEFAULT, CT::RgbLimited, 1, [GS::BGRX, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::NONE, false, TRC_SRGB, CT::RgbLimited, 1, [GS::BGRX, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::NONE, false, TRC_PQ, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::NONE, false, TRC_HLG, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::NONE, false, 4, CT::RgbLimited, 1, [GS::BGRX, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::NONE, false, 256 + TRC_PQ, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::NONE, true, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::NONE, true, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::NONE, true, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::NONE, true, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::NONE, true, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::NONE, true, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::I420, false, TRC_DEFAULT, CT::I420, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_Reverse"), false),
    row(F::I420, false, TRC_SRGB, CT::I420, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_Reverse"), false),
    row(F::I420, false, TRC_PQ, CT::I420Pq, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_PQ_Reverse"), false),
    row(F::I420, false, TRC_HLG, CT::I420, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_HLG_Reverse"), false),
    row(F::I420, false, 4, CT::I420, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_Reverse"), false),
    row(F::I420, false, 256 + TRC_PQ, CT::I420Pq, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_PQ_Reverse"), false),
    row(F::I420, true, TRC_DEFAULT, CT::I420, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_Reverse"), false),
    row(F::I420, true, TRC_SRGB, CT::I420, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_Reverse"), false),
    row(F::I420, true, TRC_PQ, CT::I420Pq, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_PQ_Reverse"), false),
    row(F::I420, true, TRC_HLG, CT::I420, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_HLG_Reverse"), false),
    row(F::I420, true, 4, CT::I420, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_Reverse"), false),
    row(F::I420, true, 256 + TRC_PQ, CT::I420Pq, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I420_PQ_Reverse"), false),
    row(F::NV12, false, TRC_DEFAULT, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_Reverse"), false),
    row(F::NV12, false, TRC_SRGB, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_Reverse"), false),
    row(F::NV12, false, TRC_PQ, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_PQ_Reverse"), false),
    row(F::NV12, false, TRC_HLG, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_HLG_Reverse"), false),
    row(F::NV12, false, 4, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_Reverse"), false),
    row(F::NV12, false, 256 + TRC_PQ, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_PQ_Reverse"), false),
    row(F::NV12, true, TRC_DEFAULT, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_Reverse"), false),
    row(F::NV12, true, TRC_SRGB, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_Reverse"), false),
    row(F::NV12, true, TRC_PQ, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_PQ_Reverse"), false),
    row(F::NV12, true, TRC_HLG, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_HLG_Reverse"), false),
    row(F::NV12, true, 4, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_Reverse"), false),
    row(F::NV12, true, 256 + TRC_PQ, CT::Nv12, 2, [GS::R8, GS::R8G8, GS::UNKNOWN, GS::UNKNOWN], Some(c"NV12_PQ_Reverse"), false),
    row(F::YVYU, false, TRC_DEFAULT, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, false, TRC_SRGB, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, false, TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, false, TRC_HLG, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, false, 4, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, false, 256 + TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, true, TRC_DEFAULT, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, true, TRC_SRGB, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, true, TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, true, TRC_HLG, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, true, 4, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YVYU, true, 256 + TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YVYU_Reverse"), false),
    row(F::YUY2, false, TRC_DEFAULT, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_Reverse"), false),
    row(F::YUY2, false, TRC_SRGB, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_Reverse"), false),
    row(F::YUY2, false, TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_PQ_Reverse"), false),
    row(F::YUY2, false, TRC_HLG, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_HLG_Reverse"), false),
    row(F::YUY2, false, 4, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_Reverse"), false),
    row(F::YUY2, false, 256 + TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_PQ_Reverse"), false),
    row(F::YUY2, true, TRC_DEFAULT, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_Reverse"), false),
    row(F::YUY2, true, TRC_SRGB, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_Reverse"), false),
    row(F::YUY2, true, TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_PQ_Reverse"), false),
    row(F::YUY2, true, TRC_HLG, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_HLG_Reverse"), false),
    row(F::YUY2, true, 4, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_Reverse"), false),
    row(F::YUY2, true, 256 + TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"YUY2_PQ_Reverse"), false),
    row(F::UYVY, false, TRC_DEFAULT, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, false, TRC_SRGB, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, false, TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, false, TRC_HLG, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, false, 4, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, false, 256 + TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, true, TRC_DEFAULT, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, true, TRC_SRGB, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, true, TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, true, TRC_HLG, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, true, 4, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::UYVY, true, 256 + TRC_PQ, CT::I422Pack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"UYVY_Reverse"), false),
    row(F::RGBA, false, TRC_DEFAULT, CT::RgbLimited, 1, [GS::RGBA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::RGBA, false, TRC_SRGB, CT::RgbLimited, 1, [GS::RGBA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::RGBA, false, TRC_PQ, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::RGBA, false, TRC_HLG, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::RGBA, false, 4, CT::RgbLimited, 1, [GS::RGBA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::RGBA, false, 256 + TRC_PQ, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::RGBA, true, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::RGBA, true, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::RGBA, true, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::RGBA, true, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::RGBA, true, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::RGBA, true, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRA, false, TRC_DEFAULT, CT::RgbLimited, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRA, false, TRC_SRGB, CT::RgbLimited, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRA, false, TRC_PQ, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRA, false, TRC_HLG, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRA, false, 4, CT::RgbLimited, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRA, false, 256 + TRC_PQ, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRA, true, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRA, true, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRA, true, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRA, true, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRA, true, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRA, true, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRX, false, TRC_DEFAULT, CT::RgbLimited, 1, [GS::BGRX, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRX, false, TRC_SRGB, CT::RgbLimited, 1, [GS::BGRX, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRX, false, TRC_PQ, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRX, false, TRC_HLG, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRX, false, 4, CT::RgbLimited, 1, [GS::BGRX, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRX, false, 256 + TRC_PQ, CT::RgbLimited, 1, [GS::RGBA16F, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"RGB_Limited"), false),
    row(F::BGRX, true, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRX, true, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRX, true, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRX, true, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRX, true, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::BGRX, true, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::Y800, false, TRC_DEFAULT, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Limited"), false),
    row(F::Y800, false, TRC_SRGB, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Limited"), false),
    row(F::Y800, false, TRC_PQ, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Limited"), false),
    row(F::Y800, false, TRC_HLG, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Limited"), false),
    row(F::Y800, false, 4, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Limited"), false),
    row(F::Y800, false, 256 + TRC_PQ, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Limited"), false),
    row(F::Y800, true, TRC_DEFAULT, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Full"), false),
    row(F::Y800, true, TRC_SRGB, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Full"), false),
    row(F::Y800, true, TRC_PQ, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Full"), false),
    row(F::Y800, true, TRC_HLG, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Full"), false),
    row(F::Y800, true, 4, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Full"), false),
    row(F::Y800, true, 256 + TRC_PQ, CT::Y800, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"Y800_Full"), false),
    row(F::I444, false, TRC_DEFAULT, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, false, TRC_SRGB, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, false, TRC_PQ, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, false, TRC_HLG, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, false, 4, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, false, 256 + TRC_PQ, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, true, TRC_DEFAULT, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, true, TRC_SRGB, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, true, TRC_PQ, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, true, TRC_HLG, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, true, 4, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::I444, true, 256 + TRC_PQ, CT::I444, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I444_Reverse"), false),
    row(F::BGR3, false, TRC_DEFAULT, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Limited"), false),
    row(F::BGR3, false, TRC_SRGB, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Limited"), false),
    row(F::BGR3, false, TRC_PQ, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Limited"), false),
    row(F::BGR3, false, TRC_HLG, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Limited"), false),
    row(F::BGR3, false, 4, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Limited"), false),
    row(F::BGR3, false, 256 + TRC_PQ, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Limited"), false),
    row(F::BGR3, true, TRC_DEFAULT, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Full"), false),
    row(F::BGR3, true, TRC_SRGB, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Full"), false),
    row(F::BGR3, true, TRC_PQ, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Full"), false),
    row(F::BGR3, true, TRC_HLG, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Full"), false),
    row(F::BGR3, true, 4, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Full"), false),
    row(F::BGR3, true, 256 + TRC_PQ, CT::Bgr3, 1, [GS::R8, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"BGR3_Full"), false),
    row(F::I422, false, TRC_DEFAULT, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, false, TRC_SRGB, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, false, TRC_PQ, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, false, TRC_HLG, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, false, 4, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, false, 256 + TRC_PQ, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, true, TRC_DEFAULT, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, true, TRC_SRGB, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, true, TRC_PQ, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, true, TRC_HLG, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, true, 4, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I422, true, 256 + TRC_PQ, CT::I422, 3, [GS::R8, GS::R8, GS::R8, GS::UNKNOWN], Some(c"I422_Reverse"), false),
    row(F::I40A, false, TRC_DEFAULT, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, false, TRC_SRGB, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, false, TRC_PQ, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, false, TRC_HLG, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, false, 4, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, false, 256 + TRC_PQ, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, true, TRC_DEFAULT, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, true, TRC_SRGB, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, true, TRC_PQ, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, true, TRC_HLG, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, true, 4, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I40A, true, 256 + TRC_PQ, CT::I420A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I40A_Reverse"), false),
    row(F::I42A, false, TRC_DEFAULT, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, false, TRC_SRGB, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, false, TRC_PQ, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, false, TRC_HLG, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, false, 4, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, false, 256 + TRC_PQ, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, true, TRC_DEFAULT, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, true, TRC_SRGB, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, true, TRC_PQ, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, true, TRC_HLG, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, true, 4, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::I42A, true, 256 + TRC_PQ, CT::I422A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"I42A_Reverse"), false),
    row(F::YUVA, false, TRC_DEFAULT, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, false, TRC_SRGB, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, false, TRC_PQ, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, false, TRC_HLG, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, false, 4, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, false, 256 + TRC_PQ, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, true, TRC_DEFAULT, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, true, TRC_SRGB, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, true, TRC_PQ, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, true, TRC_HLG, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, true, 4, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::YUVA, true, 256 + TRC_PQ, CT::I444A, 4, [GS::R8, GS::R8, GS::R8, GS::R8], Some(c"YUVA_Reverse"), false),
    row(F::AYUV, false, TRC_DEFAULT, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, false, TRC_SRGB, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, false, TRC_PQ, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, false, TRC_HLG, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, false, 4, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, false, 256 + TRC_PQ, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, true, TRC_DEFAULT, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, true, TRC_SRGB, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, true, TRC_PQ, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, true, TRC_HLG, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, true, 4, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::AYUV, true, 256 + TRC_PQ, CT::I444APack, 1, [GS::BGRA, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"AYUV_Reverse"), false),
    row(F::I010, false, TRC_DEFAULT, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_SRGB_Reverse"), true),
    row(F::I010, false, TRC_SRGB, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_SRGB_Reverse"), true),
    row(F::I010, false, TRC_PQ, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_PQ_2020_709_Reverse"), true),
    row(F::I010, false, TRC_HLG, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_HLG_2020_709_Reverse"), true),
    row(F::I010, false, 4, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_SRGB_Reverse"), true),
    row(F::I010, false, 256 + TRC_PQ, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_PQ_2020_709_Reverse"), true),
    row(F::I010, true, TRC_DEFAULT, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_SRGB_Reverse"), true),
    row(F::I010, true, TRC_SRGB, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_SRGB_Reverse"), true),
    row(F::I010, true, TRC_PQ, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_PQ_2020_709_Reverse"), true),
    row(F::I010, true, TRC_HLG, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_HLG_2020_709_Reverse"), true),
    row(F::I010, true, 4, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_SRGB_Reverse"), true),
    row(F::I010, true, 256 + TRC_PQ, CT::I010, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I010_PQ_2020_709_Reverse"), true),
    row(F::P010, false, TRC_DEFAULT, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_SRGB_Reverse"), true),
    row(F::P010, false, TRC_SRGB, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_SRGB_Reverse"), true),
    row(F::P010, false, TRC_PQ, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_PQ_2020_709_Reverse"), true),
    row(F::P010, false, TRC_HLG, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_HLG_2020_709_Reverse"), true),
    row(F::P010, false, 4, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_SRGB_Reverse"), true),
    row(F::P010, false, 256 + TRC_PQ, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_PQ_2020_709_Reverse"), true),
    row(F::P010, true, TRC_DEFAULT, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_SRGB_Reverse"), true),
    row(F::P010, true, TRC_SRGB, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_SRGB_Reverse"), true),
    row(F::P010, true, TRC_PQ, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_PQ_2020_709_Reverse"), true),
    row(F::P010, true, TRC_HLG, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_HLG_2020_709_Reverse"), true),
    row(F::P010, true, 4, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_SRGB_Reverse"), true),
    row(F::P010, true, 256 + TRC_PQ, CT::P010, 2, [GS::R16, GS::RG16, GS::UNKNOWN, GS::UNKNOWN], Some(c"P010_PQ_2020_709_Reverse"), true),
    row(F::I210, false, TRC_DEFAULT, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_Reverse"), true),
    row(F::I210, false, TRC_SRGB, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_Reverse"), true),
    row(F::I210, false, TRC_PQ, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_PQ_Reverse"), true),
    row(F::I210, false, TRC_HLG, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_HLG_Reverse"), true),
    row(F::I210, false, 4, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_Reverse"), true),
    row(F::I210, false, 256 + TRC_PQ, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_PQ_Reverse"), true),
    row(F::I210, true, TRC_DEFAULT, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_Reverse"), true),
    row(F::I210, true, TRC_SRGB, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_Reverse"), true),
    row(F::I210, true, TRC_PQ, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_PQ_Reverse"), true),
    row(F::I210, true, TRC_HLG, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_HLG_Reverse"), true),
    row(F::I210, true, 4, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_Reverse"), true),
    row(F::I210, true, 256 + TRC_PQ, CT::I422P10le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I210_PQ_Reverse"), true),
    row(F::I412, false, TRC_DEFAULT, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_Reverse"), true),
    row(F::I412, false, TRC_SRGB, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_Reverse"), true),
    row(F::I412, false, TRC_PQ, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_PQ_Reverse"), true),
    row(F::I412, false, TRC_HLG, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_HLG_Reverse"), true),
    row(F::I412, false, 4, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_Reverse"), true),
    row(F::I412, false, 256 + TRC_PQ, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_PQ_Reverse"), true),
    row(F::I412, true, TRC_DEFAULT, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_Reverse"), true),
    row(F::I412, true, TRC_SRGB, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_Reverse"), true),
    row(F::I412, true, TRC_PQ, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_PQ_Reverse"), true),
    row(F::I412, true, TRC_HLG, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_HLG_Reverse"), true),
    row(F::I412, true, 4, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_Reverse"), true),
    row(F::I412, true, 256 + TRC_PQ, CT::I444P12le, 3, [GS::R16, GS::R16, GS::R16, GS::UNKNOWN], Some(c"I412_PQ_Reverse"), true),
    row(F::YA2L, false, TRC_DEFAULT, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, false, TRC_SRGB, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, false, TRC_PQ, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, false, TRC_HLG, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, false, 4, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, false, 256 + TRC_PQ, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, true, TRC_DEFAULT, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, true, TRC_SRGB, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, true, TRC_PQ, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, true, TRC_HLG, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, true, 4, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::YA2L, true, 256 + TRC_PQ, CT::I444P12leA, 4, [GS::R16, GS::R16, GS::R16, GS::R16], Some(c"YA2L_Reverse"), true),
    row(F::P216, false, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, false, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, false, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, false, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, false, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, false, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, true, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, true, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, true, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, true, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, true, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P216, true, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, false, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, false, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, false, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, false, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, false, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, false, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, true, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, true, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, true, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, true, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, true, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::P416, true, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::V210, false, TRC_DEFAULT, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_SRGB_Reverse"), false),
    row(F::V210, false, TRC_SRGB, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_SRGB_Reverse"), false),
    row(F::V210, false, TRC_PQ, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_PQ_2020_709_Reverse"), false),
    row(F::V210, false, TRC_HLG, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_HLG_2020_709_Reverse"), false),
    row(F::V210, false, 4, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_SRGB_Reverse"), false),
    row(F::V210, false, 256 + TRC_PQ, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_PQ_2020_709_Reverse"), false),
    row(F::V210, true, TRC_DEFAULT, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_SRGB_Reverse"), false),
    row(F::V210, true, TRC_SRGB, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_SRGB_Reverse"), false),
    row(F::V210, true, TRC_PQ, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_PQ_2020_709_Reverse"), false),
    row(F::V210, true, TRC_HLG, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_HLG_2020_709_Reverse"), false),
    row(F::V210, true, 4, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_SRGB_Reverse"), false),
    row(F::V210, true, 256 + TRC_PQ, CT::V210, 1, [GS::R10G10B10A2, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"V210_PQ_2020_709_Reverse"), false),
    row(F::R10L, false, TRC_DEFAULT, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_SRGB_Limited_Reverse"), false),
    row(F::R10L, false, TRC_SRGB, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_SRGB_Limited_Reverse"), false),
    row(F::R10L, false, TRC_PQ, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_PQ_2020_709_Limited_Reverse"), false),
    row(F::R10L, false, TRC_HLG, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_HLG_2020_709_Limited_Reverse"), false),
    row(F::R10L, false, 4, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_SRGB_Limited_Reverse"), false),
    row(F::R10L, false, 256 + TRC_PQ, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_PQ_2020_709_Limited_Reverse"), false),
    row(F::R10L, true, TRC_DEFAULT, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_SRGB_Full_Reverse"), false),
    row(F::R10L, true, TRC_SRGB, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_SRGB_Full_Reverse"), false),
    row(F::R10L, true, TRC_PQ, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_PQ_2020_709_Full_Reverse"), false),
    row(F::R10L, true, TRC_HLG, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_HLG_2020_709_Full_Reverse"), false),
    row(F::R10L, true, 4, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_SRGB_Full_Reverse"), false),
    row(F::R10L, true, 256 + TRC_PQ, CT::R10l, 1, [GS::BGRA_UNORM, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], Some(c"R10L_PQ_2020_709_Full_Reverse"), false),
    row(F::R10L + 1, false, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, false, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, false, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, false, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, false, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, false, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, true, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, true, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, true, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, true, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, true, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(F::R10L + 1, true, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, false, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, false, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, false, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, false, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, false, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, false, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, true, TRC_DEFAULT, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, true, TRC_SRGB, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, true, TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, true, TRC_HLG, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, true, 4, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
    row(-1, true, 256 + TRC_PQ, CT::None, 0, [GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN, GS::UNKNOWN], None, false),
];

/// `size_table` of the C test: the plane widths and heights of each convert
/// type (in `ConvertType::ALL` order) at each of `SIZES`.
#[rustfmt::skip]
static SIZE_TABLE: [[([u32; PLANES], [u32; PLANES]); 14]; 21] = [
    // None
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([0, 0, 0, 0], [0, 0, 0, 0]),
    ],
    // Nv12
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 0, 0], [1, 1, 0, 0]),
        ([2, 1, 0, 0], [2, 1, 0, 0]),
        ([3, 2, 0, 0], [3, 2, 0, 0]),
        ([1281, 641, 0, 0], [721, 361, 0, 0]),
        ([1920, 960, 0, 0], [1080, 540, 0, 0]),
        ([1431655765, 715827883, 0, 0], [1431655765, 715827883, 0, 0]),
        ([1431655766, 715827883, 0, 0], [7, 4, 0, 0]),
        ([u32::MAX - 5, 2147483645, 0, 0], [u32::MAX - 5, 2147483645, 0, 0]),
        ([u32::MAX - 4, 2147483646, 0, 0], [u32::MAX - 4, 2147483646, 0, 0]),
        ([u32::MAX - 1, 2147483647, 0, 0], [u32::MAX - 1, 2147483647, 0, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [1, 1, 0, 0]),
        ([1, 1, 0, 0], [u32::MAX, 0, 0, 0]),
    ],
    // I420
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 0], [1, 1, 1, 0]),
        ([2, 1, 1, 0], [2, 1, 1, 0]),
        ([3, 2, 2, 0], [3, 2, 2, 0]),
        ([1281, 641, 641, 0], [721, 361, 361, 0]),
        ([1920, 960, 960, 0], [1080, 540, 540, 0]),
        ([1431655765, 715827883, 715827883, 0], [1431655765, 715827883, 715827883, 0]),
        ([1431655766, 715827883, 715827883, 0], [7, 4, 4, 0]),
        ([u32::MAX - 5, 2147483645, 2147483645, 0], [u32::MAX - 5, 2147483645, 2147483645, 0]),
        ([u32::MAX - 4, 2147483646, 2147483646, 0], [u32::MAX - 4, 2147483646, 2147483646, 0]),
        ([u32::MAX - 1, 2147483647, 2147483647, 0], [u32::MAX - 1, 2147483647, 2147483647, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [1, 1, 1, 0]),
        ([1, 1, 1, 0], [u32::MAX, 0, 0, 0]),
    ],
    // I420Pq
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 0], [1, 1, 1, 0]),
        ([2, 1, 1, 0], [2, 1, 1, 0]),
        ([3, 2, 2, 0], [3, 2, 2, 0]),
        ([1281, 641, 641, 0], [721, 361, 361, 0]),
        ([1920, 960, 960, 0], [1080, 540, 540, 0]),
        ([1431655765, 715827883, 715827883, 0], [1431655765, 715827883, 715827883, 0]),
        ([1431655766, 715827883, 715827883, 0], [7, 4, 4, 0]),
        ([u32::MAX - 5, 2147483645, 2147483645, 0], [u32::MAX - 5, 2147483645, 2147483645, 0]),
        ([u32::MAX - 4, 2147483646, 2147483646, 0], [u32::MAX - 4, 2147483646, 2147483646, 0]),
        ([u32::MAX - 1, 2147483647, 2147483647, 0], [u32::MAX - 1, 2147483647, 2147483647, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [1, 1, 1, 0]),
        ([1, 1, 1, 0], [u32::MAX, 0, 0, 0]),
    ],
    // I420A
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 1], [1, 1, 1, 1]),
        ([2, 1, 1, 2], [2, 1, 1, 2]),
        ([3, 2, 2, 3], [3, 2, 2, 3]),
        ([1281, 641, 641, 1281], [721, 361, 361, 721]),
        ([1920, 960, 960, 1920], [1080, 540, 540, 1080]),
        ([1431655765, 715827883, 715827883, 1431655765], [1431655765, 715827883, 715827883, 1431655765]),
        ([1431655766, 715827883, 715827883, 1431655766], [7, 4, 4, 7]),
        ([u32::MAX - 5, 2147483645, 2147483645, u32::MAX - 5], [u32::MAX - 5, 2147483645, 2147483645, u32::MAX - 5]),
        ([u32::MAX - 4, 2147483646, 2147483646, u32::MAX - 4], [u32::MAX - 4, 2147483646, 2147483646, u32::MAX - 4]),
        ([u32::MAX - 1, 2147483647, 2147483647, u32::MAX - 1], [u32::MAX - 1, 2147483647, 2147483647, u32::MAX - 1]),
        ([u32::MAX, 0, 0, u32::MAX], [u32::MAX, 0, 0, u32::MAX]),
        ([u32::MAX, 0, 0, u32::MAX], [1, 1, 1, 1]),
        ([1, 1, 1, 1], [u32::MAX, 0, 0, u32::MAX]),
    ],
    // I422
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 0], [1, 1, 1, 0]),
        ([2, 1, 1, 0], [2, 2, 2, 0]),
        ([3, 2, 2, 0], [3, 3, 3, 0]),
        ([1281, 641, 641, 0], [721, 721, 721, 0]),
        ([1920, 960, 960, 0], [1080, 1080, 1080, 0]),
        ([1431655765, 715827883, 715827883, 0], [1431655765, 1431655765, 1431655765, 0]),
        ([1431655766, 715827883, 715827883, 0], [7, 7, 7, 0]),
        ([u32::MAX - 5, 2147483645, 2147483645, 0], [u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, 0]),
        ([u32::MAX - 4, 2147483646, 2147483646, 0], [u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, 0]),
        ([u32::MAX - 1, 2147483647, 2147483647, 0], [u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, u32::MAX, u32::MAX, 0]),
        ([u32::MAX, 0, 0, 0], [1, 1, 1, 0]),
        ([1, 1, 1, 0], [u32::MAX, u32::MAX, u32::MAX, 0]),
    ],
    // I422P10le
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 0], [1, 1, 1, 0]),
        ([2, 1, 1, 0], [2, 2, 2, 0]),
        ([3, 2, 2, 0], [3, 3, 3, 0]),
        ([1281, 641, 641, 0], [721, 721, 721, 0]),
        ([1920, 960, 960, 0], [1080, 1080, 1080, 0]),
        ([1431655765, 715827883, 715827883, 0], [1431655765, 1431655765, 1431655765, 0]),
        ([1431655766, 715827883, 715827883, 0], [7, 7, 7, 0]),
        ([u32::MAX - 5, 2147483645, 2147483645, 0], [u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, 0]),
        ([u32::MAX - 4, 2147483646, 2147483646, 0], [u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, 0]),
        ([u32::MAX - 1, 2147483647, 2147483647, 0], [u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, u32::MAX, u32::MAX, 0]),
        ([u32::MAX, 0, 0, 0], [1, 1, 1, 0]),
        ([1, 1, 1, 0], [u32::MAX, u32::MAX, u32::MAX, 0]),
    ],
    // I422A
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 1], [1, 1, 1, 1]),
        ([2, 1, 1, 2], [2, 2, 2, 2]),
        ([3, 2, 2, 3], [3, 3, 3, 3]),
        ([1281, 641, 641, 1281], [721, 721, 721, 721]),
        ([1920, 960, 960, 1920], [1080, 1080, 1080, 1080]),
        ([1431655765, 715827883, 715827883, 1431655765], [1431655765, 1431655765, 1431655765, 1431655765]),
        ([1431655766, 715827883, 715827883, 1431655766], [7, 7, 7, 7]),
        ([u32::MAX - 5, 2147483645, 2147483645, u32::MAX - 5], [u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, u32::MAX - 5]),
        ([u32::MAX - 4, 2147483646, 2147483646, u32::MAX - 4], [u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, u32::MAX - 4]),
        ([u32::MAX - 1, 2147483647, 2147483647, u32::MAX - 1], [u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, u32::MAX - 1]),
        ([u32::MAX, 0, 0, u32::MAX], [u32::MAX, u32::MAX, u32::MAX, u32::MAX]),
        ([u32::MAX, 0, 0, u32::MAX], [1, 1, 1, 1]),
        ([1, 1, 1, 1], [u32::MAX, u32::MAX, u32::MAX, u32::MAX]),
    ],
    // I422Pack
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 0, 0, 0], [1, 0, 0, 0]),
        ([1, 0, 0, 0], [2, 0, 0, 0]),
        ([2, 0, 0, 0], [3, 0, 0, 0]),
        ([641, 0, 0, 0], [721, 0, 0, 0]),
        ([960, 0, 0, 0], [1080, 0, 0, 0]),
        ([715827883, 0, 0, 0], [1431655765, 0, 0, 0]),
        ([715827883, 0, 0, 0], [7, 0, 0, 0]),
        ([2147483645, 0, 0, 0], [u32::MAX - 5, 0, 0, 0]),
        ([2147483646, 0, 0, 0], [u32::MAX - 4, 0, 0, 0]),
        ([2147483647, 0, 0, 0], [u32::MAX - 1, 0, 0, 0]),
        ([0, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([0, 0, 0, 0], [1, 0, 0, 0]),
        ([1, 0, 0, 0], [u32::MAX, 0, 0, 0]),
    ],
    // I444
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 0], [1, 1, 1, 0]),
        ([2, 2, 2, 0], [2, 2, 2, 0]),
        ([3, 3, 3, 0], [3, 3, 3, 0]),
        ([1281, 1281, 1281, 0], [721, 721, 721, 0]),
        ([1920, 1920, 1920, 0], [1080, 1080, 1080, 0]),
        ([1431655765, 1431655765, 1431655765, 0], [1431655765, 1431655765, 1431655765, 0]),
        ([1431655766, 1431655766, 1431655766, 0], [7, 7, 7, 0]),
        ([u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, 0], [u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, 0]),
        ([u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, 0], [u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, 0]),
        ([u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, 0], [u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, 0]),
        ([u32::MAX, u32::MAX, u32::MAX, 0], [u32::MAX, u32::MAX, u32::MAX, 0]),
        ([u32::MAX, u32::MAX, u32::MAX, 0], [1, 1, 1, 0]),
        ([1, 1, 1, 0], [u32::MAX, u32::MAX, u32::MAX, 0]),
    ],
    // I444P12le
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 0], [1, 1, 1, 0]),
        ([2, 2, 2, 0], [2, 2, 2, 0]),
        ([3, 3, 3, 0], [3, 3, 3, 0]),
        ([1281, 1281, 1281, 0], [721, 721, 721, 0]),
        ([1920, 1920, 1920, 0], [1080, 1080, 1080, 0]),
        ([1431655765, 1431655765, 1431655765, 0], [1431655765, 1431655765, 1431655765, 0]),
        ([1431655766, 1431655766, 1431655766, 0], [7, 7, 7, 0]),
        ([u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, 0], [u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, 0]),
        ([u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, 0], [u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, 0]),
        ([u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, 0], [u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, 0]),
        ([u32::MAX, u32::MAX, u32::MAX, 0], [u32::MAX, u32::MAX, u32::MAX, 0]),
        ([u32::MAX, u32::MAX, u32::MAX, 0], [1, 1, 1, 0]),
        ([1, 1, 1, 0], [u32::MAX, u32::MAX, u32::MAX, 0]),
    ],
    // I444A
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 1], [1, 1, 1, 1]),
        ([2, 2, 2, 2], [2, 2, 2, 2]),
        ([3, 3, 3, 3], [3, 3, 3, 3]),
        ([1281, 1281, 1281, 1281], [721, 721, 721, 721]),
        ([1920, 1920, 1920, 1920], [1080, 1080, 1080, 1080]),
        ([1431655765, 1431655765, 1431655765, 1431655765], [1431655765, 1431655765, 1431655765, 1431655765]),
        ([1431655766, 1431655766, 1431655766, 1431655766], [7, 7, 7, 7]),
        ([u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, u32::MAX - 5], [u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, u32::MAX - 5]),
        ([u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, u32::MAX - 4], [u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, u32::MAX - 4]),
        ([u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, u32::MAX - 1], [u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, u32::MAX - 1]),
        ([u32::MAX, u32::MAX, u32::MAX, u32::MAX], [u32::MAX, u32::MAX, u32::MAX, u32::MAX]),
        ([u32::MAX, u32::MAX, u32::MAX, u32::MAX], [1, 1, 1, 1]),
        ([1, 1, 1, 1], [u32::MAX, u32::MAX, u32::MAX, u32::MAX]),
    ],
    // I444P12leA
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 1], [1, 1, 1, 1]),
        ([2, 2, 2, 2], [2, 2, 2, 2]),
        ([3, 3, 3, 3], [3, 3, 3, 3]),
        ([1281, 1281, 1281, 1281], [721, 721, 721, 721]),
        ([1920, 1920, 1920, 1920], [1080, 1080, 1080, 1080]),
        ([1431655765, 1431655765, 1431655765, 1431655765], [1431655765, 1431655765, 1431655765, 1431655765]),
        ([1431655766, 1431655766, 1431655766, 1431655766], [7, 7, 7, 7]),
        ([u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, u32::MAX - 5], [u32::MAX - 5, u32::MAX - 5, u32::MAX - 5, u32::MAX - 5]),
        ([u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, u32::MAX - 4], [u32::MAX - 4, u32::MAX - 4, u32::MAX - 4, u32::MAX - 4]),
        ([u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, u32::MAX - 1], [u32::MAX - 1, u32::MAX - 1, u32::MAX - 1, u32::MAX - 1]),
        ([u32::MAX, u32::MAX, u32::MAX, u32::MAX], [u32::MAX, u32::MAX, u32::MAX, u32::MAX]),
        ([u32::MAX, u32::MAX, u32::MAX, u32::MAX], [1, 1, 1, 1]),
        ([1, 1, 1, 1], [u32::MAX, u32::MAX, u32::MAX, u32::MAX]),
    ],
    // I444APack
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 0, 0, 0], [1, 0, 0, 0]),
        ([2, 0, 0, 0], [2, 0, 0, 0]),
        ([3, 0, 0, 0], [3, 0, 0, 0]),
        ([1281, 0, 0, 0], [721, 0, 0, 0]),
        ([1920, 0, 0, 0], [1080, 0, 0, 0]),
        ([1431655765, 0, 0, 0], [1431655765, 0, 0, 0]),
        ([1431655766, 0, 0, 0], [7, 0, 0, 0]),
        ([u32::MAX - 5, 0, 0, 0], [u32::MAX - 5, 0, 0, 0]),
        ([u32::MAX - 4, 0, 0, 0], [u32::MAX - 4, 0, 0, 0]),
        ([u32::MAX - 1, 0, 0, 0], [u32::MAX - 1, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [1, 0, 0, 0]),
        ([1, 0, 0, 0], [u32::MAX, 0, 0, 0]),
    ],
    // Y800
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 0, 0, 0], [1, 0, 0, 0]),
        ([2, 0, 0, 0], [2, 0, 0, 0]),
        ([3, 0, 0, 0], [3, 0, 0, 0]),
        ([1281, 0, 0, 0], [721, 0, 0, 0]),
        ([1920, 0, 0, 0], [1080, 0, 0, 0]),
        ([1431655765, 0, 0, 0], [1431655765, 0, 0, 0]),
        ([1431655766, 0, 0, 0], [7, 0, 0, 0]),
        ([u32::MAX - 5, 0, 0, 0], [u32::MAX - 5, 0, 0, 0]),
        ([u32::MAX - 4, 0, 0, 0], [u32::MAX - 4, 0, 0, 0]),
        ([u32::MAX - 1, 0, 0, 0], [u32::MAX - 1, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [1, 0, 0, 0]),
        ([1, 0, 0, 0], [u32::MAX, 0, 0, 0]),
    ],
    // RgbLimited
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 0, 0, 0], [1, 0, 0, 0]),
        ([2, 0, 0, 0], [2, 0, 0, 0]),
        ([3, 0, 0, 0], [3, 0, 0, 0]),
        ([1281, 0, 0, 0], [721, 0, 0, 0]),
        ([1920, 0, 0, 0], [1080, 0, 0, 0]),
        ([1431655765, 0, 0, 0], [1431655765, 0, 0, 0]),
        ([1431655766, 0, 0, 0], [7, 0, 0, 0]),
        ([u32::MAX - 5, 0, 0, 0], [u32::MAX - 5, 0, 0, 0]),
        ([u32::MAX - 4, 0, 0, 0], [u32::MAX - 4, 0, 0, 0]),
        ([u32::MAX - 1, 0, 0, 0], [u32::MAX - 1, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [1, 0, 0, 0]),
        ([1, 0, 0, 0], [u32::MAX, 0, 0, 0]),
    ],
    // Bgr3
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([3, 0, 0, 0], [1, 0, 0, 0]),
        ([6, 0, 0, 0], [2, 0, 0, 0]),
        ([9, 0, 0, 0], [3, 0, 0, 0]),
        ([3843, 0, 0, 0], [721, 0, 0, 0]),
        ([5760, 0, 0, 0], [1080, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [1431655765, 0, 0, 0]),
        ([2, 0, 0, 0], [7, 0, 0, 0]),
        ([4294967278, 0, 0, 0], [u32::MAX - 5, 0, 0, 0]),
        ([4294967281, 0, 0, 0], [u32::MAX - 4, 0, 0, 0]),
        ([u32::MAX - 5, 0, 0, 0], [u32::MAX - 1, 0, 0, 0]),
        ([u32::MAX - 2, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([u32::MAX - 2, 0, 0, 0], [1, 0, 0, 0]),
        ([3, 0, 0, 0], [u32::MAX, 0, 0, 0]),
    ],
    // I010
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 1, 0], [1, 1, 1, 0]),
        ([2, 1, 1, 0], [2, 1, 1, 0]),
        ([3, 2, 2, 0], [3, 2, 2, 0]),
        ([1281, 641, 641, 0], [721, 361, 361, 0]),
        ([1920, 960, 960, 0], [1080, 540, 540, 0]),
        ([1431655765, 715827883, 715827883, 0], [1431655765, 715827883, 715827883, 0]),
        ([1431655766, 715827883, 715827883, 0], [7, 4, 4, 0]),
        ([u32::MAX - 5, 2147483645, 2147483645, 0], [u32::MAX - 5, 2147483645, 2147483645, 0]),
        ([u32::MAX - 4, 2147483646, 2147483646, 0], [u32::MAX - 4, 2147483646, 2147483646, 0]),
        ([u32::MAX - 1, 2147483647, 2147483647, 0], [u32::MAX - 1, 2147483647, 2147483647, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [1, 1, 1, 0]),
        ([1, 1, 1, 0], [u32::MAX, 0, 0, 0]),
    ],
    // P010
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 1, 0, 0], [1, 1, 0, 0]),
        ([2, 1, 0, 0], [2, 1, 0, 0]),
        ([3, 2, 0, 0], [3, 2, 0, 0]),
        ([1281, 641, 0, 0], [721, 361, 0, 0]),
        ([1920, 960, 0, 0], [1080, 540, 0, 0]),
        ([1431655765, 715827883, 0, 0], [1431655765, 715827883, 0, 0]),
        ([1431655766, 715827883, 0, 0], [7, 4, 0, 0]),
        ([u32::MAX - 5, 2147483645, 0, 0], [u32::MAX - 5, 2147483645, 0, 0]),
        ([u32::MAX - 4, 2147483646, 0, 0], [u32::MAX - 4, 2147483646, 0, 0]),
        ([u32::MAX - 1, 2147483647, 0, 0], [u32::MAX - 1, 2147483647, 0, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [1, 1, 0, 0]),
        ([1, 1, 0, 0], [u32::MAX, 0, 0, 0]),
    ],
    // V210
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([4, 0, 0, 0], [1, 0, 0, 0]),
        ([4, 0, 0, 0], [2, 0, 0, 0]),
        ([4, 0, 0, 0], [3, 0, 0, 0]),
        ([856, 0, 0, 0], [721, 0, 0, 0]),
        ([1280, 0, 0, 0], [1080, 0, 0, 0]),
        ([954437180, 0, 0, 0], [1431655765, 0, 0, 0]),
        ([954437180, 0, 0, 0], [7, 0, 0, 0]),
        ([2863311528, 0, 0, 0], [u32::MAX - 5, 0, 0, 0]),
        ([0, 0, 0, 0], [u32::MAX - 4, 0, 0, 0]),
        ([0, 0, 0, 0], [u32::MAX - 1, 0, 0, 0]),
        ([0, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([0, 0, 0, 0], [1, 0, 0, 0]),
        ([4, 0, 0, 0], [u32::MAX, 0, 0, 0]),
    ],
    // R10l
    [
        ([0, 0, 0, 0], [0, 0, 0, 0]),
        ([1, 0, 0, 0], [1, 0, 0, 0]),
        ([2, 0, 0, 0], [2, 0, 0, 0]),
        ([3, 0, 0, 0], [3, 0, 0, 0]),
        ([1281, 0, 0, 0], [721, 0, 0, 0]),
        ([1920, 0, 0, 0], [1080, 0, 0, 0]),
        ([1431655765, 0, 0, 0], [1431655765, 0, 0, 0]),
        ([1431655766, 0, 0, 0], [7, 0, 0, 0]),
        ([u32::MAX - 5, 0, 0, 0], [u32::MAX - 5, 0, 0, 0]),
        ([u32::MAX - 4, 0, 0, 0], [u32::MAX - 4, 0, 0, 0]),
        ([u32::MAX - 1, 0, 0, 0], [u32::MAX - 1, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [u32::MAX, 0, 0, 0]),
        ([u32::MAX, 0, 0, 0], [1, 0, 0, 0]),
        ([1, 0, 0, 0], [u32::MAX, 0, 0, 0]),
    ],
];

fn size_row(convert_type: ConvertType, size: usize) -> ([u32; PLANES], [u32; PLANES]) {
    let index = CT::ALL.iter().position(|&t| t == convert_type).unwrap();
    SIZE_TABLE[index][size]
}

#[track_caller]
fn check_plan(plan: &Plan, row: &Row, sizes: ([u32; PLANES], [u32; PLANES]), what: &str) {
    assert_eq!(plan.convert_type, row.convert_type, "{what}: convert type");
    assert_eq!(
        plan.channel_count, row.channel_count,
        "{what}: channel count"
    );
    assert_eq!(plan.linear, row.linear, "{what}: linear");
    for c in 0..PLANES {
        let expected = Plane {
            width: sizes.0[c],
            height: sizes.1[c],
            format: row.plane_format[c],
        };
        assert_eq!(plan.planes[c], expected, "{what}: plane {c}");
    }
    assert_eq!(plan.technique, row.technique, "{what}: technique");
}

fn technique_of(plan: &Plan) -> Option<&'static str> {
    plan.technique.map(|t| t.to_str().unwrap())
}

// --------------------------------------------------------------------------
// The C test's cases

#[test]
fn steps_table_covers_every_input() {
    assert_eq!(CONVERSION_TABLE.len(), FORMATS.len() * 2 * TRCS.len());
    let mut i = 0;
    for format in FORMATS {
        for full_range in [false, true] {
            for trc in TRCS {
                let row = &CONVERSION_TABLE[i];
                assert_eq!(
                    (row.format, row.full_range, row.trc),
                    (format, full_range, trc),
                    "row {i}"
                );
                i += 1;
            }
        }
    }
}

#[test]
fn steps_full_table() {
    for row in &CONVERSION_TABLE {
        for (s, &(width, height)) in SIZES.iter().enumerate() {
            let what = format!(
                "format {} range {} trc {} size {width} x {height}",
                row.format, row.full_range, row.trc
            );
            let plan = plan(row.format, row.full_range, row.trc, width, height);
            check_plan(&plan, row, size_row(row.convert_type, s), &what);
        }
    }
}

#[test]
fn steps_unused_planes_zero() {
    for row in &CONVERSION_TABLE {
        let plan = plan(row.format, row.full_range, row.trc, 1920, 1080);
        for c in plan.channel_count..PLANES {
            assert_eq!(
                plan.planes[c],
                Plane::NONE,
                "format {} plane {c}",
                row.format
            );
        }
        if plan.convert_type == CT::None {
            assert_eq!(plan.channel_count, 0);
        } else {
            assert!(
                (1..=PLANES).contains(&plan.channel_count),
                "format {}",
                row.format
            );
        }
    }
}

#[test]
fn steps_nv12_1080p() {
    let p = plan(F::NV12, false, TRC_DEFAULT, 1920, 1080);
    assert_eq!(p.convert_type, CT::Nv12);
    assert_eq!(p.channel_count, 2);
    assert_eq!(
        p.planes[0],
        Plane {
            width: 1920,
            height: 1080,
            format: GS::R8
        }
    );
    assert_eq!(
        p.planes[1],
        Plane {
            width: 960,
            height: 540,
            format: GS::R8G8
        }
    );
    assert_eq!(technique_of(&p), Some("NV12_Reverse"));
    assert!(!p.linear);
}

#[test]
fn steps_half_sizes_round_up() {
    let p = plan(F::I420, false, TRC_DEFAULT, 1281, 721);
    assert_eq!(p.channel_count, 3);
    assert_eq!(p.planes.map(|p| p.width)[..3], [1281, 641, 641]);
    assert_eq!(p.planes.map(|p| p.height)[..3], [721, 361, 361]);

    // packed 4:2:2: one BGRA texel for two pixels, full height
    let p = plan(F::YUY2, false, TRC_DEFAULT, 3, 3);
    assert_eq!(p.channel_count, 1);
    assert_eq!(
        p.planes[0],
        Plane {
            width: 2,
            height: 3,
            format: GS::BGRA
        }
    );

    // planar 4:2:2: half width, full height
    let p = plan(F::I422, false, TRC_DEFAULT, 1, 1);
    assert_eq!((p.planes[1].width, p.planes[1].height), (1, 1));
}

/// Characterized, not endorsed: the size rules wrap in `u32`.
#[test]
fn steps_size_wraps() {
    // (u32::MAX + 1) / 2 is 0
    let p = plan(F::I420, false, TRC_DEFAULT, u32::MAX, u32::MAX);
    assert_eq!(p.planes[0].width, u32::MAX);
    assert_eq!(p.planes[1].width, 0);
    assert_eq!(p.planes[1].height, 0);
    assert_eq!(
        plan(F::UYVY, false, TRC_DEFAULT, u32::MAX, 5).planes[0].width,
        0
    );
    assert_eq!(
        plan(F::UYVY, false, TRC_DEFAULT, u32::MAX - 1, 5).planes[0].width,
        0x7FFF_FFFF
    );

    // BGR3: three R8 texels a pixel, wrapping from 0x55555556 on
    assert_eq!(
        plan(F::BGR3, true, TRC_DEFAULT, 0x5555_5555, 7).planes[0].width,
        0xFFFF_FFFF
    );
    let p = plan(F::BGR3, true, TRC_DEFAULT, 0x5555_5556, 7);
    assert_eq!(
        p.planes[0],
        Plane {
            width: 2,
            height: 7,
            format: GS::R8
        }
    );

    // v210: four texels for six pixels, (width + 5) / 6 * 4
    let p = plan(F::V210, false, TRC_DEFAULT, 1920, 1080);
    assert_eq!(
        (p.planes[0].width, p.planes[0].format),
        (1280, GS::R10G10B10A2)
    );
    assert_eq!(plan(F::V210, false, TRC_DEFAULT, 1, 1).planes[0].width, 4);
    assert_eq!(
        plan(F::V210, false, TRC_DEFAULT, u32::MAX - 5, 1).planes[0].width,
        2_863_311_528
    );
    assert_eq!(
        plan(F::V210, false, TRC_DEFAULT, u32::MAX - 4, 1).planes[0].width,
        0
    );
    assert_eq!(
        plan(F::V210, false, TRC_DEFAULT, u32::MAX, 1).planes[0].width,
        0
    );
}

#[test]
fn steps_rgb_limited() {
    let rgb = [F::NONE, F::RGBA, F::BGRA, F::BGRX];
    let sdr_format = [GS::BGRX, GS::RGBA, GS::BGRA, GS::BGRX];
    for (i, &format) in rgb.iter().enumerate() {
        let p = plan(format, true, TRC_DEFAULT, 64, 32);
        assert_eq!(p.convert_type, CT::None);
        assert_eq!(p.channel_count, 0);
        assert_eq!(p.technique, None);

        for trc in TRCS {
            let t = trc & 0xFF;
            let p = plan(format, false, trc, 64, 32);
            assert_eq!(p.convert_type, CT::RgbLimited);
            assert_eq!(p.channel_count, 1);
            let expected = if t == TRC_PQ || t == TRC_HLG {
                GS::RGBA16F
            } else {
                sdr_format[i]
            };
            assert_eq!(
                p.planes[0],
                Plane {
                    width: 64,
                    height: 32,
                    format: expected
                },
                "{format} {trc}"
            );
            assert_eq!(technique_of(&p), Some("RGB_Limited"));
            assert!(!p.linear);
        }
    }
}

#[test]
fn steps_i420_transfers() {
    let p = plan(F::I420, false, TRC_PQ, 2, 2);
    assert_eq!(
        (p.convert_type, technique_of(&p)),
        (CT::I420Pq, Some("I420_PQ_Reverse"))
    );
    let p = plan(F::I420, false, TRC_HLG, 2, 2);
    assert_eq!(
        (p.convert_type, technique_of(&p)),
        (CT::I420, Some("I420_HLG_Reverse"))
    );
    let p = plan(F::I420, false, TRC_SRGB, 2, 2);
    assert_eq!(
        (p.convert_type, technique_of(&p)),
        (CT::I420, Some("I420_Reverse"))
    );
    let p = plan(F::NV12, true, TRC_HLG, 2, 2);
    assert_eq!(
        (p.convert_type, technique_of(&p)),
        (CT::Nv12, Some("NV12_HLG_Reverse"))
    );
}

#[test]
fn steps_trc_modulo_256() {
    let p = plan(F::I420, false, 256 + TRC_PQ, 2, 2);
    assert_eq!(
        (p.convert_type, technique_of(&p)),
        (CT::I420Pq, Some("I420_PQ_Reverse"))
    );
    assert_eq!(
        plan(F::BGRA, false, 256 + TRC_HLG, 2, 2).planes[0].format,
        GS::RGBA16F
    );
    assert_eq!(
        technique_of(&plan(F::P010, false, 4, 2, 2)),
        Some("P010_SRGB_Reverse")
    );
}

#[test]
fn steps_range_techniques() {
    let cases = [
        (F::Y800, TRC_DEFAULT, "Y800_Full", "Y800_Limited"),
        (F::BGR3, TRC_PQ, "BGR3_Full", "BGR3_Limited"),
        (
            F::R10L,
            TRC_DEFAULT,
            "R10L_SRGB_Full_Reverse",
            "R10L_SRGB_Limited_Reverse",
        ),
        (
            F::R10L,
            TRC_SRGB,
            "R10L_SRGB_Full_Reverse",
            "R10L_SRGB_Limited_Reverse",
        ),
        (
            F::R10L,
            TRC_PQ,
            "R10L_PQ_2020_709_Full_Reverse",
            "R10L_PQ_2020_709_Limited_Reverse",
        ),
        (
            F::R10L,
            TRC_HLG,
            "R10L_HLG_2020_709_Full_Reverse",
            "R10L_HLG_2020_709_Limited_Reverse",
        ),
        (F::I444, TRC_DEFAULT, "I444_Reverse", "I444_Reverse"),
    ];
    for (format, trc, full, limited) in cases {
        assert_eq!(technique_of(&plan(format, true, trc, 8, 8)), Some(full));
        assert_eq!(technique_of(&plan(format, false, trc, 8, 8)), Some(limited));
    }
}

#[test]
fn steps_no_conversion() {
    for format in [F::P216, F::P416, F::R10L + 1, -1] {
        for full_range in [false, true] {
            let p = plan(format, full_range, TRC_PQ, 1920, 1080);
            assert_eq!(p.convert_type, CT::None);
            assert_eq!(p.channel_count, 0);
            assert_eq!(p.technique, None);
            assert!(!p.linear);
        }
    }
}

#[test]
fn steps_linear_formats() {
    for format in FORMATS {
        let expected = [F::I010, F::P010, F::I210, F::I412, F::YA2L].contains(&format);
        assert_eq!(
            plan(format, true, TRC_HLG, 4, 4).linear,
            expected,
            "{format}"
        );
        assert_eq!(
            plan(format, false, TRC_DEFAULT, 4, 4).linear,
            expected,
            "{format}"
        );
    }
}

#[test]
fn steps_alpha_and_16_bit_planes() {
    let p = plan(F::I40A, false, TRC_DEFAULT, 5, 5);
    assert_eq!(p.channel_count, 4);
    assert_eq!((p.planes[3].width, p.planes[3].height), (5, 5));
    assert_eq!((p.planes[1].width, p.planes[1].height), (3, 3));
    let p = plan(F::YA2L, false, TRC_DEFAULT, 5, 5);
    assert_eq!(p.channel_count, 4);
    assert!(p.planes.iter().all(|p| p.format == GS::R16));
    let p = plan(F::P010, false, TRC_DEFAULT, 5, 5);
    assert_eq!(p.channel_count, 2);
    assert_eq!(
        (p.planes[0].format, p.planes[1].format),
        (GS::R16, GS::RG16)
    );
    let p = plan(F::AYUV, false, TRC_DEFAULT, 5, 5);
    assert_eq!((p.channel_count, p.planes[0].format), (1, GS::BGRA));
    assert_eq!(
        plan(F::R10L, true, TRC_DEFAULT, 5, 5).planes[0].format,
        GS::BGRA_UNORM
    );
}

// --------------------------------------------------------------------------
// Edge cases

/// No conversion: what a value outside the enum, P216, P416 and full-range
/// RGB give.
const NO_PLAN: Plan = Plan {
    convert_type: CT::None,
    channel_count: 0,
    planes: [Plane::NONE; PLANES],
    technique: None,
    linear: false,
};

#[test]
fn edge_convert_type_values() {
    for (i, t) in CT::ALL.into_iter().enumerate() {
        assert_eq!(usize::try_from(t.to_c()).unwrap(), i, "{t:?}");
    }
}

#[test]
fn edge_half() {
    assert_eq!(half(0), 0);
    assert_eq!(half(1), 1);
    assert_eq!(half(2), 1);
    assert_eq!(half(u32::MAX - 1), 0x7FFF_FFFF);
    assert_eq!(half(u32::MAX), 0);
}

/// Each piece agrees with the plan it is part of, for every format, range
/// and transfer byte.
#[test]
fn edge_plan_is_its_pieces() {
    for format in -2..=27 {
        for full_range in [false, true] {
            for trc in 0..=255u8 {
                let p = plan(format, full_range, i32::from(trc), 1281, 721);
                let t = convert_type(format, full_range, trc);
                assert_eq!(p.convert_type, t);
                assert_eq!(
                    (p.channel_count, p.planes),
                    planes(t, format, trc, 1281, 721)
                );
                assert_eq!(p.technique, technique(format, full_range, trc));
                assert_eq!(p.linear, linear(format));
            }
        }
    }
}

/// A technique exactly when there is a conversion, and the count of planes
/// set is the channel count.
#[test]
fn edge_technique_and_planes_follow_the_convert_type() {
    for format in -2..=27 {
        for full_range in [false, true] {
            for trc in 0..=255 {
                let p = plan(format, full_range, trc, 7, 5);
                assert_eq!(
                    p.technique.is_some(),
                    p.convert_type != CT::None,
                    "{format} {full_range} {trc}"
                );
                for (c, plane) in p.planes.iter().enumerate() {
                    assert_eq!(
                        plane.format != GS::UNKNOWN,
                        c < p.channel_count,
                        "{format} plane {c}"
                    );
                }
            }
        }
    }
}

/// Every technique is one of `format_conversion.effect`'s.
#[test]
fn edge_techniques_are_in_the_effect() {
    let effect = include_str!("../../../../libobs/data/format_conversion.effect");
    let defined: Vec<&str> = effect
        .lines()
        .filter_map(|line| line.trim().strip_prefix("technique "))
        .map(str::trim)
        .collect();
    let mut seen = 0;
    for format in -2..=27 {
        for full_range in [false, true] {
            for trc in 0..=4u8 {
                if let Some(t) = technique(format, full_range, trc) {
                    let name = t.to_str().unwrap();
                    assert!(
                        defined.contains(&name),
                        "{name} is not in format_conversion.effect"
                    );
                    seen += 1;
                }
            }
        }
    }
    assert!(seen > 0);
}

/// `convert_video_format()` for the four RGB formats.
#[test]
fn edge_rgb_limited_format() {
    for trc in 0..=255u8 {
        let hdr = trc == 2 || trc == 3;
        for (format, sdr) in [
            (F::NONE, GS::BGRX),
            (F::RGBA, GS::RGBA),
            (F::BGRA, GS::BGRA),
            (F::BGRX, GS::BGRX),
        ] {
            let expected = if hdr { GS::RGBA16F } else { sdr };
            assert_eq!(rgb_limited_format(format, trc), expected, "{format} {trc}");
        }
    }
}

/// The plane sizes of a convert type, in `u64` and reduced modulo 2^32:
/// full, half ((n + 1) / 2 with n + 1 wrapping first), three times, and
/// v210's (n + 5) / 6 * 4 with n + 5 wrapping first.
fn model_sizes(t: ConvertType, width: u32, height: u32) -> [(u32, u32); PLANES] {
    const M: u64 = 0xFFFF_FFFF;
    let (w, h) = (u64::from(width), u64::from(height));
    let half = |n: u64| u32::try_from(((n + 1) & M) / 2).unwrap();
    let wrap = |n: u64| u32::try_from(n & M).unwrap();
    let full = (width, height);
    let half_w = (half(w), height);
    let half_both = (half(w), half(h));
    let z = (0, 0);
    match t {
        CT::None => [z; 4],
        CT::I422Pack => [half_w, z, z, z],
        CT::I420 | CT::I420Pq | CT::I010 => [full, half_both, half_both, z],
        CT::I420A => [full, half_both, half_both, full],
        CT::I422 | CT::I422P10le => [full, half_w, half_w, z],
        CT::I422A => [full, half_w, half_w, full],
        CT::Nv12 | CT::P010 => [full, half_both, z, z],
        CT::I444 | CT::I444P12le => [full, full, full, z],
        CT::I444A | CT::I444P12leA => [full; 4],
        CT::Y800 | CT::RgbLimited | CT::I444APack | CT::R10l => [full, z, z, z],
        CT::Bgr3 => [(wrap(w * 3), height), z, z, z],
        CT::V210 => [(wrap(((w + 5) & M) / 6 * 4), height), z, z, z],
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// Any value outside `enum video_format` is no conversion.
    #[test]
    fn edge_formats_outside_the_enum(
        format in any::<i32>().prop_filter("outside the enum", |f| !(F::NONE..=F::R10L).contains(f)),
        full_range in any::<bool>(),
        trc in any::<i32>(),
        width in any::<u32>(),
        height in any::<u32>(),
    ) {
        prop_assert_eq!(plan(format, full_range, trc, width, height), NO_PLAN);
    }

    /// The transfer counts modulo 256, for any `i32`.
    #[test]
    fn edge_trc_modulo_256(
        format in -2..=27i32,
        full_range in any::<bool>(),
        trc in any::<i32>(),
        width in any::<u32>(),
        height in any::<u32>(),
    ) {
        prop_assert_eq!(
            plan(format, full_range, trc, width, height),
            plan(format, full_range, trc.rem_euclid(256), width, height)
        );
    }

    /// The sizes of every format at any `u32` size, against the model.
    #[test]
    fn edge_sizes_any_u32(
        format in F::NONE..=F::R10L,
        full_range in any::<bool>(),
        trc in 0..=4i32,
        width in prop_oneof![any::<u32>(), (u32::MAX - 8)..=u32::MAX, 0..=8u32],
        height in prop_oneof![any::<u32>(), (u32::MAX - 8)..=u32::MAX, 0..=8u32],
    ) {
        let p = plan(format, full_range, trc, width, height);
        let sizes = p.planes.map(|plane| (plane.width, plane.height));
        prop_assert_eq!(sizes, model_sizes(p.convert_type, width, height));
    }

    /// The size does not change the convert type, the formats, the
    /// technique or the linear flag.
    #[test]
    fn edge_size_changes_only_sizes(
        format in -2..=27i32,
        full_range in any::<bool>(),
        trc in 0..=4i32,
        width in any::<u32>(),
        height in any::<u32>(),
    ) {
        let p = plan(format, full_range, trc, width, height);
        let q = plan(format, full_range, trc, 1920, 1080);
        prop_assert_eq!(p.convert_type, q.convert_type);
        prop_assert_eq!(p.channel_count, q.channel_count);
        prop_assert_eq!(p.planes.map(|p| p.format), q.planes.map(|p| p.format));
        prop_assert_eq!(p.technique, q.technique);
        prop_assert_eq!(p.linear, q.linear);
    }
}

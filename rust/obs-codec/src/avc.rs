//! Safe core of `libobs/obs-avc.c`: H.264 (AVC) Annex B helpers.
//!
//! Every function walks the NAL units the way the C code does (see
//! [`nal_units`]), so packets that are not clean Annex B (no start code,
//! zero runs, garbage before the first code) give the same result as in C.

use obs_util::bitstream::BitstreamReader;

use crate::nal::{self, Bucket, LengthPrefixed, SplitHeaders, UnitRating, nal_units};

pub const NAL_SLICE: u8 = 1;
pub const NAL_SLICE_IDR: u8 = 5;
pub const NAL_SEI: u8 = 6;
pub const NAL_SPS: u8 = 7;
pub const NAL_PPS: u8 = 8;

/// `nal_unit_type`, the low 5 bits of the header.
fn kind(header: u8) -> u8 {
    header & 0x1f
}

/// `obs_avc_keyframe`: whether the first slice in `data` is an IDR slice.
/// Units before it (SPS, PPS, SEI, ...) are skipped; no slice is `false`.
#[must_use]
pub fn keyframe(data: &[u8]) -> bool {
    let _ = (data, kind(0), rate(0), nal_units(data), Bucket::Header);
    let _ = (
        has_start_code as fn(&[u8]) -> bool,
        nal::packet_priority(data, 0, rate),
    );
    todo!()
}

/// `compute_avc_keyframe_priority`: an IDR unit makes a keyframe, and the
/// unit's priority is `header >> 5`, its `nal_ref_idc` plus the
/// `forbidden_zero_bit` above it (0..=7).
fn rate(header: u8) -> UnitRating {
    UnitRating {
        keyframe: kind(header) == NAL_SLICE_IDR,
        priority: i32::from(header >> 5),
    }
}

/// `obs_parse_avc_packet_priority`: `priority` raised to the highest
/// `header >> 5` of any unit in `data`.
#[must_use]
pub fn packet_priority(data: &[u8], priority: i32) -> i32 {
    let _ = (data, priority);
    todo!()
}

/// `serialize_avc_data`: converts Annex B `data` to AVCC, starting from the
/// source packet's `keyframe` and `priority`.
#[must_use]
pub fn to_avcc(data: &[u8], keyframe: bool, priority: i32) -> LengthPrefixed {
    let _ = (data, keyframe, priority);
    todo!()
}

/// `has_start_code`: whether `data` opens with `00 00 01` or `00 00 00 01`.
/// Needs at least 4 bytes, as the C version reads `data[3]`.
fn has_start_code(data: &[u8]) -> bool {
    if data[0] != 0 || data[1] != 0 {
        return false;
    }
    data[2] == 1 || (data[2] == 0 && data[3] == 1)
}

/// `get_ue_golomb`: an Exp-Golomb `ue(v)`, with the leading-zero count
/// capped at 31 (#17), as `uint8_t`.
fn ue_golomb(gb: &mut BitstreamReader<'_>) -> u8 {
    let mut i = 0;
    while i < 32 && gb.read_bits(1) == 0 {
        i += 1;
    }
    let i = i.min(31);
    // C: (uint8_t)(bitstream_reader_read_bits(gb, i) + (1u << i) - 1u),
    // where read_bits keeps only the low 8 bits.
    (u32::from(gb.read_bits(i))
        .wrapping_add(1u32 << i)
        .wrapping_sub(1)) as u8
}

/// The fields `get_sps_high_params` reads for the High profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpsHighParams {
    pub chroma_format_idc: u8,
    pub bit_depth_luma: u8,
    pub bit_depth_chroma: u8,
}

/// `get_sps_high_params`: `sps` is the SPS after its NAL header byte.
/// Emulation-prevention bytes (`00 00 03`) are dropped first.
#[must_use]
pub fn sps_high_params(sps: &[u8]) -> SpsHighParams {
    let _ = (sps, ue_golomb as fn(&mut BitstreamReader<'_>) -> u8);
    todo!()
}

/// What `obs_parse_avc_header` returns for `data`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvcHeader {
    /// Returns 0 and leaves `*header` untouched: `data` is 6 bytes or
    /// fewer, or it is Annex B without an SPS (of at least 4 bytes) and a
    /// PPS.
    None,
    /// `data` does not start with a start code: it is copied as is.
    Copy,
    /// An `AVCDecoderConfigurationRecord` built from the last SPS and the
    /// last PPS.
    Record(Vec<u8>),
}

/// `obs_parse_avc_header`.
#[must_use]
pub fn parse_header(data: &[u8]) -> AvcHeader {
    let _ = data;
    todo!()
}

/// `obs_extract_avc_headers`: SPS and PPS units go to `header`, SEI to
/// `sei`, everything else to `packet`, each with its start code.
#[must_use]
pub fn extract_headers(data: &[u8]) -> SplitHeaders {
    let _ = data;
    todo!()
}

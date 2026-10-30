//! Safe core of `libobs/obs-output-caption.c.h`: the caption plan of
//! `add_caption()` in `obs-output.c`.
//!
//! `add_caption()` runs it for each encoded video packet that a queued
//! caption goes into: the CEA-608 pairs that sources queue with
//! `obs_output_caption()`, or a page of caption text from
//! `obs_output_output_caption_text2()`. `obs-output.c` keeps the caption
//! track lookup and its log, every deque pop, `bzalloc()` and `bfree()`,
//! every libcaption call (`sei_*`, `cea708_*`, `caption_frame_*`,
//! `extract_buffer_from_sei()`, `metadata_obu()`), the new packet's
//! `DARRAY`, `obs_encoder_packet_release()` and the HEVC log, in their
//! order. It passes the values here. These keep no state and call nothing.
//!
//! - [`begin`]: the [`Carrier`] the encoder's codec gets, and whether a
//!   packet of that priority takes a caption at all;
//! - [`hevc_sei_header`]: for HEVC, the Annex B start code of the packet and
//!   the NAL unit header of the caption's suffix SEI;
//! - [`cc_keep`]: which CEA-608 triplets from the caption queue go into the
//!   CEA-708 frame.
//!
//! The codec name comes from the encoder's `info.codec`, the priority and
//! the packet from the encoder, and the triplets from sources (DeckLink,
//! plugins); the caption text that obs-websocket's `SendStreamCaption`
//! reaches does not pass through here. Nothing here can panic on any input.
//!
//! No intentional differences. [`hevc_sei_header`] takes a slice, so a
//! packet shorter than its start code and 2 header bytes gives `None`; the
//! C ABI shim reads the bytes the C reads (#3074).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// The carrier a caption gets in the packet: `OBS_OUTPUT_CAPTION_*` in
/// `obs-output-caption.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Carrier {
    /// `OBS_OUTPUT_CAPTION_NONE`: the codec carries no captions; the caption
    /// still leaves the queue.
    None = 0,
    /// `OBS_OUTPUT_CAPTION_AVC`: an SEI NAL unit after the packet.
    Avc = 1,
    /// `OBS_OUTPUT_CAPTION_HEVC`: a suffix SEI NAL unit after the packet.
    Hevc = 2,
    /// `OBS_OUTPUT_CAPTION_AV1`: a metadata OBU after the packet.
    Av1 = 3,
}

impl Carrier {
    /// The `OBS_OUTPUT_CAPTION_*` value.
    #[must_use]
    pub const fn to_c(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Avc => 1,
            Self::Hevc => 2,
            Self::Av1 => 3,
        }
    }
}

/// [`begin`]'s answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Begin {
    /// The packet takes a caption (priority 1 or below); else the caption
    /// stays queued and `add_caption()` returns false.
    pub takes: bool,
    /// The carrier, set either way.
    pub carrier: Carrier,
}

/// `obs_output_caption_begin()`: the carrier for a packet whose encoder's
/// codec is `codec` (`info.codec`, without its NUL): `h264` gives
/// [`Carrier::Avc`], `av1` [`Carrier::Av1`], and `hevc`
/// [`Carrier::Hevc`] when `hevc` is true (`ENABLE_HEVC`); any other name,
/// compared exactly, gives [`Carrier::None`]. A packet of `priority` above 1
/// takes no caption.
#[must_use]
pub fn begin(codec: &[u8], hevc: bool, priority: i32) -> Begin {
    let carrier = match codec {
        b"h264" => Carrier::Avc,
        b"av1" => Carrier::Av1,
        b"hevc" if hevc => Carrier::Hevc,
        _ => Carrier::None,
    };
    Begin {
        takes: priority <= 1,
        carrier,
    }
}

/// `obs_output_caption_hevc_sei_header()`: the 2-byte NAL unit header of
/// the caption's suffix SEI, from the header after the packet's Annex B
/// start code (`00 00 01`, tried first, else `00 00 00 01`): NAL type 40
/// (`SUFFIX_SEI_NUT`) with the first byte's last bit (the top bit of
/// `nuh_layer_id`) and the whole second byte kept. `None` when `data`
/// starts with neither start code, or ends before the 2 header bytes.
#[must_use]
pub fn hevc_sei_header(data: &[u8]) -> Option<[u8; 2]> {
    let header = match data {
        [0, 0, 1, header @ ..] | [0, 0, 0, 1, header @ ..] => header,
        _ => return None,
    };
    let [first, second, ..] = *header else {
        return None;
    };
    Some([(SUFFIX_SEI_NAL_TYPE << 1) | (first & 0x01), second])
}

/// `SUFFIX_SEI_NUT`, the NAL type of the caption's SEI.
const SUFFIX_SEI_NAL_TYPE: u8 = 40;

/// `obs_output_caption_cc_keep()`: the `cc_data` word (`cc_data_1 << 8 |
/// cc_data_2`) of a CEA-608 triplet from the caption queue (`cc_valid` and
/// `cc_type`, then the 2 data bytes), if it goes into the CEA-708 frame:
/// its `cc_type` is 0 (field 1), and the word is not padding (`0x8080`),
/// not 0 and has odd parity in both bytes. The other bits of the first
/// byte, `cc_valid` included, are not looked at.
#[must_use]
pub fn cc_keep(triplet: [u8; 3]) -> Option<u16> {
    let [kind, first, second] = triplet;
    // only send cea 608
    if kind & 0x3 != 0 {
        return None;
    }
    let word = u16::from_be_bytes([first, second]);
    // padding
    if word == 0x8080 || word == 0 || !odd_parity(first) || !odd_parity(second) {
        return None;
    }
    Some(word)
}

/// `eia608_parity_byte(b) == b` in libcaption: `b` has an odd number of bits
/// set (bit 7 is the parity bit).
const fn odd_parity(b: u8) -> bool {
    b.count_ones() & 1 == 1
}

//! Safe core of `libobs/obs-av1.c`: AV1 OBU helpers.
//!
//! An AV1 temporal unit is a sequence of OBUs, each a header byte, an
//! optional extension byte, an optional leb128 size, and the payload. The
//! walk here is the one in `obs-av1.c` with its bounds checks: an OBU never
//! extends past the end of the buffer, and every OBU is at least its header
//! byte, so the walk always advances and stays in bounds.

pub const OBU_SEQUENCE_HEADER: u8 = 1;
pub const OBU_TEMPORAL_DELIMITER: u8 = 2;
pub const OBU_FRAME_HEADER: u8 = 3;
pub const OBU_METADATA: u8 = 5;
pub const OBU_FRAME: u8 = 6;
pub const OBU_PADDING: u8 = 15;

/// `METADATA_TYPE_ITUT_T35` from `obs-av1.h`.
pub const METADATA_TYPE_ITUT_T35: u8 = 4;

/// `leb128`: reads at most 8 bytes, stopping after the first byte without
/// the continuation bit or at the end of `buf`. Returns the value and the
/// number of bytes read.
fn leb128(buf: &[u8]) -> (u64, usize) {
    let mut value = 0u64;
    let mut len = 0;
    for (i, &byte) in buf.iter().take(8).enumerate() {
        len += 1;
        value |= u64::from(byte & 0x7f) << (i * 7);
        if byte & 0x80 == 0 {
            break;
        }
    }
    (value, len)
}

/// `get_bits`: `count` bits of `val` starting `n` bits from the top.
fn get_bits(val: u8, n: u32, count: u32) -> u32 {
    (u32::from(val) >> (8 - n - count)) & ((1 << (count - 1)) * 2 - 1)
}

/// One OBU, as offsets from where it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Obu {
    /// `obu_type`, bits 6..3 of the header byte.
    pub kind: u8,
    /// Header byte, extension byte and size field: where the payload
    /// starts.
    pub header_len: usize,
    /// The payload size, cut to what the buffer holds.
    pub size: usize,
}

impl Obu {
    /// The whole OBU: header and payload.
    #[must_use]
    pub fn len(&self) -> usize {
        self.header_len + self.size
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// `parse_obu_header` for a non-empty `buf`.
fn parse_obu_header(buf: &[u8]) -> Obu {
    let size = buf.len();
    let kind = get_bits(buf[0], 1, 4) as u8;
    let extension_flag = get_bits(buf[0], 5, 1) != 0;
    let has_size_field = get_bits(buf[0], 6, 1) != 0;

    // An extension byte that is not there: the OBU is what is left.
    let mut header_len = (1 + usize::from(extension_flag)).min(size);
    let obu_size = if has_size_field {
        let (value, len) = leb128(&buf[header_len..]);
        header_len += len;
        value
    } else {
        // sz - 1 - obu_extension_flag
        (size - header_len) as u64
    };
    // C: *obu_size = (size_t)leb128(...), then never more than the buffer
    // holds.
    let size = (obu_size as usize).min(size - header_len);
    Obu {
        kind,
        header_len,
        size,
    }
}

/// The OBUs of `data` with their offsets, in order.
pub fn obus(data: &[u8]) -> impl Iterator<Item = (usize, Obu)> + '_ {
    let mut pos = 0;
    core::iter::from_fn(move || {
        if pos >= data.len() {
            return None;
        }
        let obu = parse_obu_header(&data[pos..]);
        let at = pos;
        pos += obu.len();
        Some((at, obu))
    })
}

/// `obs_av1_keyframe`: decided by the first frame or frame header OBU with
/// a payload. A shown existing frame is never a key frame; otherwise
/// `frame_type` 0 (`KEY_FRAME`) is.
#[must_use]
pub fn keyframe(data: &[u8]) -> bool {
    for (at, obu) in obus(data) {
        if obu.size != 0 && (obu.kind == OBU_FRAME || obu.kind == OBU_FRAME_HEADER) {
            let val = data[at + obu.header_len];
            if get_bits(val, 0, 1) == 0 {
                // !show_existing_frame
                return get_bits(val, 1, 2) == 0; // frame_type
            }
            return false;
        }
    }
    false
}

/// `obs_extract_av1_headers`: every OBU stays in `packet`; sequence header
/// and metadata OBUs are also copied to `header`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Av1Headers {
    pub packet: Vec<u8>,
    pub header: Vec<u8>,
}

/// The header output of `obs_extract_av1_headers`: the sequence header and
/// metadata OBUs of `data`, in order, as slices of `data`.
///
/// The packet output is all of `data`: the walk starts at 0, every OBU is at
/// least one byte and the last one is cut to the end of the buffer, so the
/// OBUs cover every byte. The C ABI shim copies both outputs straight into
/// their `bmalloc` buffers from these slices (#1304).
pub fn header_obus(data: &[u8]) -> impl Iterator<Item = &[u8]> + '_ {
    obus(data)
        .filter(|(_, obu)| obu.kind == OBU_METADATA || obu.kind == OBU_SEQUENCE_HEADER)
        .map(move |(at, obu)| &data[at..at + obu.len()])
}

/// `obs_extract_av1_headers`.
#[must_use]
pub fn extract_headers(data: &[u8]) -> Av1Headers {
    Av1Headers {
        packet: data.to_vec(),
        header: header_obus(data).flatten().copied().collect(),
    }
}

/// `metadata_obu`'s trailing bits after the payload.
pub const METADATA_TRAILING_BITS: u8 = 0x80;

/// The bytes of `metadata_obu` in front of the payload: the header byte,
/// the leb128 size field (`encode_uleb128`, at most 10 bytes) and
/// `metadata_type`, as a buffer and its used length. A metadata OBU is
/// these bytes, the payload and [`METADATA_TRAILING_BITS`].
#[must_use]
pub fn metadata_obu_prefix(payload_len: usize, metadata_type: u8) -> ([u8; 12], usize) {
    let mut out = [0u8; 12];
    // obu_type METADATA, obu_has_size_field set
    out[0] = (OBU_METADATA << 3) | (1 << 1);
    let mut n = 1;
    // C: int64_t size_field = 1 + source_bufsize + 1;
    let mut val = 1 + payload_len as u64 + 1;
    loop {
        let b = (val & 0x7f) as u8;
        val >>= 7;
        if val == 0 {
            out[n] = b;
            n += 1;
            break;
        }
        out[n] = b | 0x80;
        n += 1;
    }
    out[n] = metadata_type;
    (out, n + 1)
}

/// `metadata_obu`: a metadata OBU carrying `payload` as `metadata_type`:
/// header byte, leb128 size, type, payload, and the `0x80` trailing bits.
#[must_use]
pub fn metadata_obu(payload: &[u8], metadata_type: u8) -> Vec<u8> {
    let (prefix, n) = metadata_obu_prefix(payload.len(), metadata_type);
    [&prefix[..n], payload, &[METADATA_TRAILING_BITS]].concat()
}

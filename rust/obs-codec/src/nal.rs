//! Safe core of `libobs/obs-nal.c`: the Annex B start-code search shared by
//! the AVC and HEVC helpers.
//!
//! The C code is FFmpeg's `ff_avc_find_startcode_internal`, which scans a
//! word at a time after an alignment prologue. The word scan reports the
//! earliest `00 00 01` it can see, in byte order, so its result is the same
//! as a plain byte scan, whatever the buffer's alignment. What the C
//! code does decide is which positions are searched: a start code is only
//! found at `i` when `i + 3 < len`, so one in the last three bytes, with
//! nothing after it, is not reported.
//!
//! The scan here first tests the [`NEAR`] positions at the start 8 at a
//! time, then 64 at a time to skip data with no start code, then 8 at a
//! time to locate one, then single bytes at the end. A block is only
//! skipped when the exact three-byte pattern is absent, and the other steps
//! test that pattern too, so the result is that of a byte scan.

/// The positions [`block_has_zero_pair`] and [`block_has_startcode`] test in
/// one step.
const BLOCK: usize = 64;

/// The positions [`word_startcode`] tests in one step.
const WORD: usize = 8;

/// The positions tested a word at a time before the block skip. Parameter
/// sets and SEI units are short, so the next start code is often this
/// close, and a block test there costs more than it skips (#1301).
const NEAR: usize = 4 * WORD;

/// Whether `00 00` begins at any of the first [`BLOCK`] positions of
/// `window`.
fn block_has_zero_pair(window: &[u8; BLOCK + 2]) -> bool {
    let (a, b) = (&window[..BLOCK], &window[1..=BLOCK]);
    let mut hit = false;
    for i in 0..BLOCK {
        hit |= (a[i] | b[i]) == 0;
    }
    hit
}

/// Whether `00 00 01` begins at any of the first [`BLOCK`] positions of
/// `window`.
fn block_has_startcode(window: &[u8; BLOCK + 2]) -> bool {
    let (a, b, c) = (&window[..BLOCK], &window[1..=BLOCK], &window[2..]);
    // A fixed-length OR over every position, which compiles to vector compares.
    let mut hit = false;
    for i in 0..BLOCK {
        hit |= (a[i] | b[i] | (c[i] ^ 1)) == 0;
    }
    hit
}

/// The first of the first [`WORD`] positions of `window` where `00 00 01`
/// begins.
// PERF GUARD #1301 (ci/perf-guards.toml): keep the zero-byte prefilter.
fn word_startcode(window: &[u8; WORD + 2]) -> Option<usize> {
    const ONES: u64 = u64::from_ne_bytes([0x01; WORD]);
    const HIGH: u64 = u64::from_ne_bytes([0x80; WORD]);
    let word = |at: usize| u64::from_le_bytes(window[at..at + WORD].try_into().unwrap());
    // A start code begins with a zero byte. Without one in the first word,
    // none begins at these positions: one load instead of three (#1301).
    let first = word(0);
    if first.wrapping_sub(ONES) & !first & HIGH == 0 {
        return None;
    }
    // A byte of `x` is zero exactly where a start code begins.
    let x = first | word(1) | (word(2) ^ ONES);
    // The lowest flagged byte is the first zero byte. Borrows can flag bytes above it.
    let zero = x.wrapping_sub(ONES) & !x & HIGH;
    (zero != 0).then(|| (zero.trailing_zeros() / 8) as usize)
}

/// `ff_avc_find_startcode_internal`: the first `i` with
/// `data[i..i + 3] == [0, 0, 1]` and `i + 3 < data.len()`, else
/// `data.len()`.
// PERF GUARD #1301 (ci/perf-guards.toml): keep the NEAR word steps before
// the block skip. Measure {avc,hevc}/keyframe before you change the steps.
fn find_startcode_internal(data: &[u8]) -> usize {
    let searchable = data.len().saturating_sub(3);
    let mut i = 0;
    while i < NEAR && i + WORD <= searchable {
        if let Some(at) = word_startcode(data[i..i + WORD + 2].try_into().unwrap()) {
            return i + at;
        }
        i += WORD;
    }
    // Skip whole blocks that hold no start code. Most have no zero pair
    // either, which takes two loads per vector to see where the full pattern
    // takes three.
    while i + BLOCK <= searchable {
        let window = data[i..i + BLOCK + 2].try_into().unwrap();
        if block_has_zero_pair(window) && block_has_startcode(window) {
            break;
        }
        i += BLOCK;
    }
    while i + WORD <= searchable {
        if let Some(at) = word_startcode(data[i..i + WORD + 2].try_into().unwrap()) {
            return i + at;
        }
        i += WORD;
    }
    (i..searchable)
        .find(|&i| data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1)
        .unwrap_or(data.len())
}

/// `obs_nal_find_startcode`: the offset of the first start code in `data`,
/// or `data.len()` if there is none.
///
/// A 3-byte `00 00 01` preceded by a zero is reported at that zero, so a
/// 4-byte `00 00 00 01` code is found at its first byte. Only one zero is
/// folded in, and never one before the start of `data`.
#[must_use]
pub fn find_startcode(data: &[u8]) -> usize {
    let out = find_startcode_internal(data);
    if 0 < out && out < data.len() && data[out - 1] == 0 {
        out - 1
    } else {
        out
    }
}

/// One NAL unit of an Annex B buffer, as offsets into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nal {
    /// Where the search for this unit started: its start code, or for the
    /// first unit, the first start code in the buffer.
    pub code_start: usize,
    /// The NAL header, just after the start code.
    pub start: usize,
    /// The next start code, or the end of the buffer.
    pub end: usize,
}

impl Nal {
    /// The first header byte.
    #[must_use]
    pub fn header(&self, data: &[u8]) -> u8 {
        data[self.start]
    }
}

/// The NAL units of `data`, in order.
///
/// This is the loop every function in `obs-avc.c` and `obs-hevc.c`
/// repeats:
///
/// ```c
/// nal_start = obs_nal_find_startcode(data, end);
/// while (true) {
///     while (nal_start < end && !*(nal_start++))
///         ;
///     if (nal_start == end)
///         break;
///     nal_end = obs_nal_find_startcode(nal_start, end);
///     ...
///     nal_start = nal_end;
/// }
/// ```
///
/// The inner loop skips zeros and then one more byte (the `01` of the start
/// code), so a unit starts after the first nonzero byte, and the walk ends
/// when that lands on the end of the buffer.
pub fn nal_units(data: &[u8]) -> impl Iterator<Item = Nal> + '_ {
    let mut pos = find_startcode(data);
    core::iter::from_fn(move || {
        let code_start = pos;
        let start = unit_start(data, pos)?;
        let end = start + find_startcode(&data[start..]);
        pos = end;
        Some(Nal {
            code_start,
            start,
            end,
        })
    })
}

/// Where the unit behind the start code at `pos` begins: after the zeros
/// and one more byte. `None` when that is the end of `data`.
fn unit_start(data: &[u8], pos: usize) -> Option<usize> {
    let zeros = data[pos..].iter().take_while(|&&byte| byte == 0).count();
    let start = pos + zeros + 1;
    (start < data.len()).then_some(start)
}

/// The first header byte of each NAL unit of `data`, in order.
///
/// The walk is that of [`nal_units`], but the search for a unit's end waits
/// until the next unit is asked for. `obs_avc_keyframe` and
/// `obs_hevc_keyframe` read the header before that search and return at the
/// first slice, so they never scan it.
///
/// The walk also uses the internal search directly. It finds `00 00 01`
/// with at least one byte after it, so the unit starts 3 bytes on: where
/// [`nal_units`] also lands after it folds in a leading zero and skips the
/// zeros. Without the fold, no loaded byte decides where the next search
/// starts (#1301: the fold's select made each unit wait for a load).
// PERF GUARD #1301 (ci/perf-guards.toml): do not share the walk with
// nal_units. The result is the same, so no test fails, but it is 1.6x slower.
pub fn nal_headers(data: &[u8]) -> impl Iterator<Item = u8> + '_ {
    let mut pos = find_startcode_internal(data);
    let mut unit = None;
    core::iter::from_fn(move || {
        if let Some(start) = unit.take() {
            pos = start + find_startcode_internal(&data[start..]);
        }
        if pos >= data.len() {
            return None;
        }
        let start = pos + 3;
        unit = Some(start);
        Some(data[start])
    })
}

/// How a codec rates one NAL unit, from its first header byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitRating {
    /// The unit makes the packet a keyframe.
    pub keyframe: bool,
    /// The unit's priority; a packet takes the highest.
    pub priority: i32,
}

/// What `obs_parse_avc_packet` / `obs_parse_hevc_packet` derive from an
/// Annex B packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LengthPrefixed {
    /// The units, each as a big-endian `u32` size and its bytes (header
    /// included, start code dropped).
    pub payload: Vec<u8>,
    /// `keyframe` after the walk: set by any keyframe unit, else unchanged.
    pub keyframe: bool,
    /// `priority` after the walk: raised to the highest unit priority.
    pub priority: i32,
}

/// `serialize_avc_data` / `serialize_hevc_data`: converts Annex B `data` to
/// length-prefixed units, starting from the source packet's `keyframe` and
/// `priority`.
#[must_use]
pub fn to_length_prefixed(
    data: &[u8],
    keyframe: bool,
    priority: i32,
    rate: impl Fn(u8) -> UnitRating,
) -> LengthPrefixed {
    let mut payload = Vec::with_capacity(data.len() + LENGTH_PREFIX_SLACK);
    let packet = write_length_prefixed(data, keyframe, priority, rate, |bytes| {
        payload.extend_from_slice(bytes);
    });
    LengthPrefixed {
        payload,
        keyframe: packet.keyframe,
        priority: packet.priority,
    }
}

/// Room to reserve past `data.len()` for the output of
/// [`write_length_prefixed`]: a unit behind a 3-byte start code grows by one
/// byte.
pub const LENGTH_PREFIX_SLACK: usize = 16;

/// [`to_length_prefixed`], with the payload passed to `write` piece by
/// piece, in order. Returns the `keyframe` and `priority` after the walk.
pub fn write_length_prefixed(
    data: &[u8],
    keyframe: bool,
    priority: i32,
    rate: impl Fn(u8) -> UnitRating,
    mut write: impl FnMut(&[u8]),
) -> UnitRating {
    let mut packet = UnitRating { keyframe, priority };
    for nal in nal_units(data) {
        let r = rate(nal.header(data));
        if r.keyframe {
            packet.keyframe = true;
        }
        // C: return priority > new_priority ? priority : new_priority;
        packet.priority = packet.priority.max(r.priority);
        let unit = &data[nal.start..nal.end];
        // C: s_wb32(s, (uint32_t)nal_size)
        write(&(unit.len() as u32).to_be_bytes());
        write(unit);
    }
    packet
}

/// `obs_parse_*_packet_priority`: `priority` raised to the highest unit
/// priority in `data`.
#[must_use]
pub fn packet_priority(data: &[u8], priority: i32, rate: impl Fn(u8) -> UnitRating) -> i32 {
    nal_units(data).fold(priority, |p, nal| p.max(rate(nal.header(data)).priority))
}

/// Where `obs_extract_*_headers` puts a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Header,
    Sei,
    Packet,
}

impl Bucket {
    /// The position of this bucket's output in [`SplitUnits::sizes`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Bucket::Packet => 0,
            Bucket::Header => 1,
            Bucket::Sei => 2,
        }
    }
}

/// Up to this many units, [`SplitUnits`] keeps their ranges on the stack.
/// Encoders write a few units per packet (parameter sets, SEI, slices).
const INLINE_UNITS: usize = 32;

/// The units of `obs_extract_*_headers` from one start-code scan, without
/// heap allocation (#1302): the output sizes, and the unit ranges of the
/// first [`INLINE_UNITS`] units. [`SplitUnits::for_each`] gives the units
/// again from those ranges, or for a packet with more units, from one more
/// scan.
pub struct SplitUnits<'a, F: Fn(u8) -> Bucket> {
    data: &'a [u8],
    bucket: F,
    units: [(Bucket, usize, usize); INLINE_UNITS],
    count: usize,
    sizes: [usize; 3],
}

impl<'a, F: Fn(u8) -> Bucket> SplitUnits<'a, F> {
    /// Scans `data` once, with `bucket` choosing from the first header byte.
    pub fn new(data: &'a [u8], bucket: F) -> Self {
        let mut out = Self {
            data,
            bucket,
            units: [(Bucket::Packet, 0, 0); INLINE_UNITS],
            count: 0,
            sizes: [0; 3],
        };
        for nal in nal_units(data) {
            let b = (out.bucket)(nal.header(data));
            out.sizes[b.index()] += nal.end - nal.code_start;
            if out.count < INLINE_UNITS {
                out.units[out.count] = (b, nal.code_start, nal.end);
            }
            out.count += 1;
        }
        out
    }

    /// The output sizes, indexed by [`Bucket::index`]: packet, header, SEI.
    #[must_use]
    pub fn sizes(&self) -> [usize; 3] {
        self.sizes
    }

    /// Every unit with its start code, in order, with its bucket.
    pub fn for_each(&self, mut f: impl FnMut(Bucket, &[u8])) {
        if self.count <= INLINE_UNITS {
            for &(b, start, end) in &self.units[..self.count] {
                f(b, &self.data[start..end]);
            }
        } else {
            for nal in nal_units(self.data) {
                f(
                    (self.bucket)(nal.header(self.data)),
                    &self.data[nal.code_start..nal.end],
                );
            }
        }
    }
}

/// `obs_extract_*_headers`: `data` split by unit, each unit with its start
/// code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SplitHeaders {
    pub packet: Vec<u8>,
    pub header: Vec<u8>,
    pub sei: Vec<u8>,
}

/// `obs_extract_*_headers`, with `bucket` choosing from the first header
/// byte. The C ABI shims use [`SplitUnits`] directly; this owned form is
/// for the tests.
#[must_use]
pub fn split_headers(data: &[u8], bucket: impl Fn(u8) -> Bucket) -> SplitHeaders {
    let units = SplitUnits::new(data, bucket);
    let [packet, header, sei] = units.sizes();
    let mut out = SplitHeaders {
        packet: Vec::with_capacity(packet),
        header: Vec::with_capacity(header),
        sei: Vec::with_capacity(sei),
    };
    units.for_each(|b, unit| match b {
        Bucket::Header => out.header.extend_from_slice(unit),
        Bucket::Sei => out.sei.extend_from_slice(unit),
        Bucket::Packet => out.packet.extend_from_slice(unit),
    });
    out
}

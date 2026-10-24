//! CEA-608 word-level encoding primitives, mirroring the vendored libcaption
//! semantics in `deps/libcaption/src/eia608.c` and `caption/eia608.h`.
//!
//! All produced `u16` values are complete `cc_data` pairs with odd parity
//! applied to both bytes, identical to what libcaption's `eia608_*` helpers
//! return (parity included, not stripped).

/// CEA-608 grid geometry, matching `caption.h`'s `SCREEN_ROWS`/`SCREEN_COLS`.
pub const SCREEN_ROWS: usize = 15;
pub const SCREEN_COLS: usize = 32;

/// `eia608_parity_byte`: returns the low 7 bits of `cc_data` with an odd
/// parity bit set in bit 7.
pub fn parity_byte(cc_data: u8) -> u8 {
    let b = cc_data & 0x7f;
    b | if b.count_ones().is_multiple_of(2) {
        0x80
    } else {
        0
    }
}

/// `eia608_parity_word`/`eia608_parity`: applies odd parity to each byte.
pub fn parity(cc_data: u16) -> u16 {
    ((parity_byte((cc_data >> 8) as u8) as u16) << 8) | parity_byte(cc_data as u8) as u16
}

/// `eia608_parity_varify`: true when both bytes already carry odd parity.
pub fn parity_verify(cc_data: u16) -> bool {
    parity(cc_data) == cc_data
}

/// `eia608_parity_strip`: drops the parity bits.
pub fn parity_strip(cc_data: u16) -> u16 {
    cc_data & 0x7f7f
}

/// `eia608_is_basicna`: both bytes hold a basic North American code
/// (0x20..=0x7f) — detectable on either paritied or stripped input because
/// only the parity bits are masked out.
pub fn is_basicna(cc_data: u16) -> bool {
    cc_data & 0x6000 != 0
}

/// The basic North American glyphs, index 0..=95 for codes 0x20..=0x7f,
/// transcribed from `deps/libcaption/src/eia608_charmap.c`.
const BASIC_NA: [char; 96] = [
    ' ', '!', '"', '#', '$', '%', '&', '\u{2019}', '(', ')', 'á', '+', ',', '-', '.', '/', '0',
    '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', '@', 'A', 'B', 'C',
    'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V',
    'W', 'X', 'Y', 'Z', '[', 'é', ']', 'í', 'ó', 'ú', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'ç', '÷',
    'Ñ', 'ñ', '█',
];

/// `_eia608_from_utf8` restricted to the basic North American set: returns
/// the code with the glyph in the **high byte** (e.g. 'A' → 0x4100), or
/// `None` for anything unmappable — matching libcaption's `0x0000` rejection.
pub fn basicna_code(c: char) -> Option<u16> {
    BASIC_NA
        .iter()
        .position(|&m| m == c)
        .map(|idx| ((0x20 + idx) as u16) << 8)
}

/// `eia608_from_utf8_1` for a basic glyph, channel 1: one char in the high
/// byte and a null (parity-encoded 0x00) low byte.
pub fn from_utf8_1(c: char) -> Option<u16> {
    basicna_code(c).map(parity)
}

/// `eia608_from_basicna`/`eia608_from_utf8_2`: two basic glyphs packed into
/// one pair.
pub fn from_basicna(a: char, b: char) -> Option<u16> {
    let (a, b) = (basicna_code(a)?, basicna_code(b)?);
    Some(parity((a & 0xff00) | ((b & 0xff00) >> 8)))
}

/// Decodes a basic pair back to (glyph, optional second glyph) — used by the
/// roll-up decoder tests; mirrors `eia608_to_index`'s basic-NA branch.
pub fn basicna_decode(cc_data: u16) -> (Option<char>, Option<char>) {
    let d = cc_data & 0x7f7f;
    if !is_basicna(d) {
        return (None, None);
    }
    let c1 = BASIC_NA
        .get(((d >> 8) as usize).wrapping_sub(0x20))
        .copied();
    let c2 = if (0x20..0x80).contains(&(d & 0x00ff)) {
        BASIC_NA.get(((d & 0x00ff) as usize) - 0x20).copied()
    } else {
        None
    };
    (c1, c2)
}

/// Control commands (`eia608_control_t`), channel 1.
pub mod control {
    pub const RESUME_CAPTION_LOADING: u16 = 0x1420;
    pub const BACKSPACE: u16 = 0x1421;
    pub const DELETE_TO_END_OF_ROW: u16 = 0x1424;
    pub const ROLL_UP_2: u16 = 0x1425;
    pub const ROLL_UP_3: u16 = 0x1426;
    pub const ROLL_UP_4: u16 = 0x1427;
    pub const RESUME_DIRECT_CAPTIONING: u16 = 0x1429;
    pub const TEXT_RESTART: u16 = 0x142a;
    pub const TEXT_RESUME_TEXT_DISPLAY: u16 = 0x142b;
    pub const ERASE_DISPLAY_MEMORY: u16 = 0x142c;
    pub const CARRIAGE_RETURN: u16 = 0x142d;
    pub const ERASE_NON_DISPLAYED_MEMORY: u16 = 0x142e;
    pub const END_OF_CAPTION: u16 = 0x142f;
}

/// `eia608_control_command` for channel 1 (`cc == 0`): the channel bits are
/// zero, so this is `parity(cmd)` for every non-tab command.
pub fn control_command(cmd: u16) -> u16 {
    parity(cmd)
}

/// `eia608_parse_control` companion used by tests: strips parity and returns
/// the command when the pair is a channel-1 control pair in the 0x1420..=0x142f
/// range this crate emits.
pub fn parse_control(cc_data: u16) -> Option<u16> {
    let cmd = parity_strip(cc_data) & 0x167f;
    (0x1420..=0x142f).contains(&cmd).then_some(cmd)
}

/// `eia608_reverse_row_map`, verbatim from `deps/libcaption/src/eia608.c`.
const REVERSE_ROW_MAP: [u16; 16] = [2, 3, 4, 5, 10, 11, 12, 13, 14, 15, 0, 6, 7, 8, 9, 1];

/// `eia608_row_column_pramble` for channel 1, no underline: positions the
/// cursor at (`row`, `col` rounded down to a multiple of four).
///
/// Mirrors the C exactly, including that the column bits and underline bit
/// are OR'ed in **after** `parity()` is applied.
pub fn row_column_preamble(row: usize, col: usize, underline: bool) -> u16 {
    let row = REVERSE_ROW_MAP[row & 0x0f];
    let x = 0x10u16 | ((col / 4) as u16);
    parity(0x1040 | ((row << 7) & 0x0700) | ((row << 5) & 0x0020))
        | ((x << 1) & 0x001e)
        | u16::from(underline)
}

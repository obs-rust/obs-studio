//! Safe core of `libobs/graphics/libnsgif/lzw.c.h`: libnsgif's GIF LZW
//! decoder.
//!
//! A [`Decoder`] reads the LZW codes of one image from GIF sub-blocks
//! ([`Decoder::init`], [`Decoder::init_map`]) and writes their values to
//! its own 4096-value stack ([`Decoder::decode`]) or, through a colour
//! table, to the caller's pixels ([`Decoder::decode_map`]). Every call takes
//! the whole input buffer; the decoder keeps offsets into it, as the C keeps
//! a pointer.
//!
//! Characterized, not endorsed (`test_nsgif_decode`), and kept as the C has
//! them:
//! - the code width grows when an entry is added at `table_size ==
//!   code_max`, and nothing is added once the table has 4096 entries;
//! - a sub-block is refused when its size byte plus its length reach the
//!   end of the data;
//! - the padding bits after the last code read as codes;
//! - a failed init changes only what the C changed before it returned.
//!
//! All arithmetic is the C's fixed-width arithmetic, which wraps where the
//! C wraps.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `LZW_CODE_MAX`: the maximum code width in bits.
pub const LZW_CODE_MAX: u8 = 12;

/// `LZW_TABLE_ENTRY_MAX`: entries in the code table, and values in the
/// output stack.
pub const TABLE_ENTRY_MAX: usize = 1 << LZW_CODE_MAX;

/// `enum lzw_result`, in the C's order.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LzwResult {
    /// `LZW_OK`.
    Ok = 0,
    /// `LZW_OK_EOD`: a zero-length sub-block.
    OkEod,
    /// `LZW_NO_MEM`.
    NoMem,
    /// `LZW_NO_DATA`: out of data.
    NoData,
    /// `LZW_EOI_CODE`: the End of Information code.
    EoiCode,
    /// `LZW_NO_COLOUR`: no colour table.
    NoColour,
    /// `LZW_BAD_ICODE`: a bad first code (or minimum code size).
    BadIcode,
    /// `LZW_BAD_PARAM`.
    BadParam,
    /// `LZW_BAD_CODE`: a code past the table.
    BadCode,
}

/// `struct lzw_table_entry`.
#[derive(Debug, Clone, Copy, Default)]
struct Entry {
    value: u8,
    first: u8,
    count: u16,
    extends: u16,
}

/// `struct lzw_read_ctx`, with offsets into the input instead of pointers.
#[derive(Debug, Clone, Copy, Default)]
struct Input {
    data_len: usize,
    data_sb_next: usize,
    sb_data: usize,
    sb_bit: usize,
    sb_bit_count: u32,
}

/// `struct lzw_ctx` without the colour table pointer, which the caller
/// keeps (the C ABI shim).
#[derive(Debug, Clone)]
pub struct Decoder {
    input: Input,
    prev_code: u16,
    prev_code_first: u16,
    prev_code_count: u16,
    initial_code_size: u8,
    code_size: u8,
    code_max: u16,
    clear_code: u16,
    eoi_code: u16,
    table_size: u16,
    output_code: u16,
    output_left: u16,
    has_transparency: bool,
    transparency_idx: u8,
    table: [Entry; TABLE_ENTRY_MAX],
    stack: [u8; TABLE_ENTRY_MAX],
}

impl Default for Decoder {
    fn default() -> Self {
        Self {
            input: Input::default(),
            prev_code: 0,
            prev_code_first: 0,
            prev_code_count: 0,
            initial_code_size: 0,
            code_size: 0,
            code_max: 0,
            clear_code: 0,
            eoi_code: 0,
            table_size: 0,
            output_code: 0,
            output_left: 0,
            has_transparency: false,
            transparency_idx: 0,
            table: [Entry::default(); TABLE_ENTRY_MAX],
            stack: [0; TABLE_ENTRY_MAX],
        }
    }
}

impl Decoder {
    /// `lzw_decode_init`: start decoding `data` at `input_pos`, the size
    /// byte of the first sub-block.
    pub fn init(&mut self, data: &[u8], minimum_code_size: u8, input_pos: usize) -> LzwResult {
        let _ = (data, minimum_code_size, input_pos);
        todo!("RED")
    }

    /// `lzw_decode_init_map` without the colour table, which the caller
    /// keeps: [`Decoder::init`], then the transparent index (none at 256 and
    /// above).
    pub fn init_map(
        &mut self,
        data: &[u8],
        minimum_code_size: u8,
        transparency_idx: u32,
        input_pos: usize,
    ) -> LzwResult {
        let _ = (data, minimum_code_size, transparency_idx, input_pos);
        todo!("RED")
    }

    /// `lzw_decode`: fill the output stack ([`Decoder::output`]); returns
    /// the result and the number of values written.
    pub fn decode(&mut self, data: &[u8]) -> (LzwResult, u32) {
        let _ = data;
        todo!("RED")
    }

    /// The output stack [`Decoder::decode`] writes.
    pub fn output(&self) -> &[u8; TABLE_ENTRY_MAX] {
        &self.stack
    }

    /// `lzw_decode_map`: write colour-mapped values to `out`; a value equal
    /// to the transparent index leaves its pixel. `None` is the C's null
    /// colour table (`LZW_NO_COLOUR`).
    pub fn decode_map(
        &mut self,
        data: &[u8],
        colour_map: Option<&[u32; 256]>,
        out: &mut [u32],
    ) -> (LzwResult, u32) {
        let _ = (data, colour_map, out);
        todo!("RED")
    }
}

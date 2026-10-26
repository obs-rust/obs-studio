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

impl Input {
    /// `lzw__block_advance`: move to the next sub-block.
    fn block_advance(&mut self, data: &[u8]) -> LzwResult {
        let next_block_pos = self.data_sb_next;
        if next_block_pos >= self.data_len {
            return LzwResult::NoData;
        }
        let Some(&size) = data.get(next_block_pos) else {
            // The caller passed a shorter buffer than at init.
            return LzwResult::NoData;
        };
        let block_size = usize::from(size);
        if next_block_pos.wrapping_add(block_size) >= self.data_len {
            return LzwResult::NoData;
        }

        self.sb_bit = 0;
        self.sb_bit_count = u32::from(size).wrapping_mul(8);

        if block_size == 0 {
            self.data_sb_next = self.data_sb_next.wrapping_add(1);
            return LzwResult::OkEod;
        }

        self.sb_data = next_block_pos.wrapping_add(1);
        self.data_sb_next = self.data_sb_next.wrapping_add(block_size).wrapping_add(1);
        LzwResult::Ok
    }

    /// The byte at `bit` of the current sub-block; 0 past the buffer (not
    /// reached: every read is inside a validated sub-block).
    fn byte(&self, data: &[u8], bit: usize) -> u32 {
        data.get(self.sb_data.wrapping_add(bit >> 3))
            .map_or(0, |&b| u32::from(b))
    }

    /// `lzw__read_code`: the next `code_size`-bit code, across sub-blocks.
    fn read_code(&mut self, data: &[u8], code_size: u8) -> Result<u16, LzwResult> {
        let mut code: u32 = 0;
        let current_bit = (self.sb_bit & 0x7) as u32;

        if self.sb_bit.wrapping_add(24) <= self.sb_bit_count as usize {
            // Fast path: three bytes from this sub-block.
            code |= self.byte(data, self.sb_bit);
            code |= self.byte(data, self.sb_bit.wrapping_add(8)) << 8;
            code |= self.byte(data, self.sb_bit.wrapping_add(16)) << 16;
            self.sb_bit = self.sb_bit.wrapping_add(usize::from(code_size));
        } else {
            // Slow path: the code spans sub-blocks.
            let byte_advance = (current_bit
                .wrapping_add(u32::from(code_size))
                .wrapping_sub(1)
                >> 3) as u8;
            let mut byte: u8 = 0;
            let room = 8u32.wrapping_sub(current_bit) as u8;
            let bits_remaining_0 = if code_size < room { code_size } else { room };
            let bits_remaining_1 = code_size.wrapping_sub(bits_remaining_0);
            let bits_used: [u8; 3] = [
                bits_remaining_0,
                if bits_remaining_1 < 8 {
                    bits_remaining_1
                } else {
                    8
                },
                bits_remaining_1.wrapping_sub(8),
            ];

            loop {
                while byte <= byte_advance && self.sb_bit < self.sb_bit_count as usize {
                    code |= self.byte(data, self.sb_bit) << (u32::from(byte) << 3);
                    let used = bits_used.get(usize::from(byte)).copied().unwrap_or(0);
                    self.sb_bit = self.sb_bit.wrapping_add(usize::from(used));
                    byte = byte.wrapping_add(1);
                }
                if byte > byte_advance {
                    break;
                }
                let res = self.block_advance(data);
                if res != LzwResult::Ok {
                    return Err(res);
                }
            }
        }

        let mask = (1u32 << code_size).wrapping_sub(1);
        Ok(((code >> current_bit) & mask) as u16)
    }
}

/// How a decode writes values: to the output stack, or through a colour
/// table to the caller's pixels.
enum Writer<'a> {
    Stack,
    Map {
        colours: &'a [u32; 256],
        out: &'a mut [u32],
    },
}

impl Decoder {
    fn entry(&self, code: u16) -> Entry {
        self.table
            .get(usize::from(code))
            .copied()
            .unwrap_or_default()
    }

    /// `lzw__handle_clear`: reset the table and read the first code after
    /// any number of clear codes.
    fn handle_clear(&mut self, data: &[u8]) -> Result<u16, LzwResult> {
        self.code_size = self.initial_code_size;
        self.code_max = (1u16 << self.initial_code_size).wrapping_sub(1);
        self.table_size = self.eoi_code.wrapping_add(1);

        let mut code;
        loop {
            code = self.input.read_code(data, self.code_size)?;
            if code != self.clear_code {
                break;
            }
        }

        if code > self.clear_code {
            return Err(LzwResult::BadIcode);
        }
        Ok(code)
    }

    /// `lzw_decode_init`: start decoding `data` at `input_pos`, the size
    /// byte of the first sub-block.
    pub fn init(&mut self, data: &[u8], minimum_code_size: u8, input_pos: usize) -> LzwResult {
        if minimum_code_size >= LZW_CODE_MAX {
            return LzwResult::BadIcode;
        }

        self.input.data_len = data.len();
        self.input.data_sb_next = input_pos;
        self.input.sb_bit = 0;
        self.input.sb_bit_count = 0;

        self.initial_code_size = minimum_code_size.wrapping_add(1);
        self.clear_code = 1u16 << minimum_code_size;
        self.eoi_code = (1u16 << minimum_code_size).wrapping_add(1);

        self.output_left = 0;

        for i in 0..self.clear_code {
            if let Some(entry) = self.table.get_mut(usize::from(i)) {
                entry.first = i as u8;
                entry.value = i as u8;
                entry.count = 1;
            }
        }

        let code = match self.handle_clear(data) {
            Ok(code) => code,
            Err(res) => return res,
        };

        let entry = self.entry(code);
        self.prev_code_first = u16::from(entry.first);
        self.prev_code_count = entry.count;
        self.prev_code = code;

        self.output_code = code;
        self.output_left = 1;

        self.has_transparency = false;
        self.transparency_idx = 0;
        LzwResult::Ok
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
        let res = self.init(data, minimum_code_size, input_pos);
        if res != LzwResult::Ok {
            return res;
        }
        self.has_transparency = transparency_idx <= 0xFF;
        self.transparency_idx = transparency_idx as u8;
        LzwResult::Ok
    }

    /// `lzw__table_add_entry`.
    fn table_add_entry(&mut self, code: u16) {
        let entry = Entry {
            value: code as u8,
            first: self.prev_code_first as u8,
            count: self.prev_code_count.wrapping_add(1),
            extends: self.prev_code,
        };
        if let Some(slot) = self.table.get_mut(usize::from(self.table_size)) {
            *slot = entry;
        }
        self.table_size = self.table_size.wrapping_add(1);
    }

    /// `lzw__write_fn` and `lzw__map_write_fn`: write up to `left` values of
    /// `code`'s record at `used`, the first ones that fit; keep the rest
    /// for the next call.
    fn write(&mut self, writer: &mut Writer<'_>, used: u32, mut code: u16, mut left: u16) -> u32 {
        let length = match writer {
            Writer::Stack => TABLE_ENTRY_MAX as u32,
            Writer::Map { out, .. } => out.len() as u32,
        };
        let space = length.wrapping_sub(used);
        let mut count = left;

        if u32::from(count) > space {
            left = (u32::from(count).wrapping_sub(space)) as u16;
            count = space as u16;
        } else {
            left = 0;
        }

        self.output_code = code;
        self.output_left = left;

        for _ in 0..left {
            code = self.entry(code).extends;
        }

        let mut pos = used.wrapping_add(u32::from(count)) as usize;
        for _ in 0..count {
            let entry = self.entry(code);
            pos = pos.wrapping_sub(1);
            match writer {
                Writer::Stack => {
                    if let Some(v) = self.stack.get_mut(pos) {
                        *v = entry.value;
                    }
                }
                Writer::Map { colours, out } => {
                    if (!self.has_transparency || entry.value != self.transparency_idx)
                        && let Some(v) = out.get_mut(pos)
                    {
                        *v = colours.get(usize::from(entry.value)).copied().unwrap_or(0);
                    }
                }
            }
            code = entry.extends;
        }
        u32::from(count)
    }

    /// `lzw__decode`: read one code and write its values.
    fn decode_one(&mut self, data: &[u8], writer: &mut Writer<'_>, written: &mut u32) -> LzwResult {
        let mut code = match self.input.read_code(data, self.code_size) {
            Ok(code) => code,
            Err(res) => return res,
        };

        if code == self.eoi_code {
            return LzwResult::EoiCode;
        } else if code > self.table_size {
            return LzwResult::BadCode;
        } else if code == self.clear_code {
            code = match self.handle_clear(data) {
                Ok(code) => code,
                Err(res) => return res,
            };
        } else if usize::from(self.table_size) < TABLE_ENTRY_MAX {
            let size = self.table_size;
            let first = if code < size {
                u16::from(self.entry(code).first)
            } else {
                self.prev_code_first
            };
            self.table_add_entry(first);

            if size == self.code_max && self.code_size < LZW_CODE_MAX {
                self.code_size = self.code_size.wrapping_add(1);
                self.code_max = (1u16 << self.code_size).wrapping_sub(1);
            }
        }

        let count = self.entry(code).count;
        let n = self.write(writer, *written, code, count);
        *written = written.wrapping_add(n);

        let entry = self.entry(code);
        self.prev_code_first = u16::from(entry.first);
        self.prev_code_count = entry.count;
        self.prev_code = code;
        LzwResult::Ok
    }

    /// Continue a partial record, then decode until `length` values are
    /// written or a result other than `Ok`.
    fn run(&mut self, data: &[u8], writer: &mut Writer<'_>, length: u32) -> (LzwResult, u32) {
        let mut written = 0u32;
        if self.output_left != 0 {
            let (code, left) = (self.output_code, self.output_left);
            written = written.wrapping_add(self.write(writer, written, code, left));
        }
        while written != length {
            let res = self.decode_one(data, writer, &mut written);
            if res != LzwResult::Ok {
                return (res, written);
            }
        }
        (LzwResult::Ok, written)
    }

    /// `lzw_decode`: fill the output stack ([`Decoder::output`]); returns
    /// the result and the number of values written.
    pub fn decode(&mut self, data: &[u8]) -> (LzwResult, u32) {
        self.run(data, &mut Writer::Stack, TABLE_ENTRY_MAX as u32)
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
        let Some(colours) = colour_map else {
            return (LzwResult::NoColour, 0);
        };
        let length = out.len() as u32;
        self.run(data, &mut Writer::Map { colours, out }, length)
    }
}

//! Tier 1: the safe core of `libobs/graphics/libnsgif/lzw.c.h`.
//!
//! The `lzw_*` cases mirror the LZW cases of
//! `test/cmocka/test_nsgif_decode.c` one to one, with the same names,
//! streams and expected values. The cmocka case
//! `lzw_code_on_byte_boundary_at_end_of_data` decodes a whole GIF through
//! libnsgif and has no counterpart here (`lzw_eoi_on_byte_boundary` covers
//! its LZW part). The C ABI cases (`LZW_BAD_PARAM`, the colour table kept
//! after a failed init) are in `lzw_parity`.
//!
//! The cases after those pin the edges of the safe API, and proptests check
//! that no input panics and the invariants.

use obs_graphics::lzw::{Decoder, LzwResult};
use proptest::prelude::*;

/// The code emitter of the cmocka cases: exact code streams with the
/// decoder's width rule.
pub struct Stream {
    raw: Vec<u8>,
    bits: usize,
    min: u32,
    pub width: u32,
    pub table_size: u32,
    clear: u32,
    eoi: u32,
    first: bool,
}

impl Stream {
    pub fn new(min: u32) -> Self {
        Self {
            raw: vec![0; 1 << 18],
            bits: 0,
            min,
            width: min + 1,
            table_size: 0,
            clear: 1 << min,
            eoi: (1 << min) + 1,
            first: false,
        }
    }

    pub fn put(&mut self, code: u32, width: u32) {
        for i in 0..width {
            if (code >> i) & 1 != 0 {
                self.raw[self.bits >> 3] |= 1 << (self.bits & 7);
            }
            self.bits += 1;
        }
    }

    pub fn clear(&mut self) {
        self.put(self.clear, self.width);
        self.width = self.min + 1;
        self.table_size = self.eoi + 1;
        self.first = true;
    }

    pub fn code(&mut self, code: u32) {
        self.put(code, self.width);
        if self.first {
            self.first = false;
        } else if self.table_size < 4096 {
            if self.table_size == (1 << self.width) - 1 && self.width < 12 {
                self.width += 1;
            }
            self.table_size += 1;
        }
    }

    pub fn eoi(&mut self) {
        self.put(self.eoi, self.width);
    }

    pub fn bits(&self) -> usize {
        self.bits
    }

    /// The bits in sub-blocks of `block_size` bytes, then a zero-length
    /// sub-block.
    pub fn pack(&self, block_size: usize) -> Vec<u8> {
        let bytes = self.bits.div_ceil(8);
        let mut out = Vec::new();
        for chunk in self.raw[..bytes].chunks(block_size) {
            out.push(chunk.len() as u8);
            out.extend_from_slice(chunk);
        }
        out.push(0);
        out
    }
}

/// Decode until a result other than `Ok`; returns it and the values.
fn decode_all(d: &mut Decoder, data: &[u8]) -> (LzwResult, Vec<u8>) {
    let mut out = Vec::new();
    loop {
        let (res, n) = d.decode(data);
        out.extend_from_slice(&d.output()[..n as usize]);
        if res != LzwResult::Ok {
            return (res, out);
        }
    }
}

fn decoder() -> Box<Decoder> {
    Box::default()
}

#[test]
fn lzw_roots_at_every_code_size() {
    let mut d = decoder();
    for min in 2..=8u32 {
        let mut st = Stream::new(min);
        st.clear();
        for v in 0..(1 << min) {
            st.code(v);
        }
        st.eoi();
        let data = st.pack(255);
        assert_eq!(d.init(&data, min as u8, 0), LzwResult::Ok);
        let (res, out) = decode_all(&mut d, &data);
        assert_eq!(res, LzwResult::EoiCode);
        assert_eq!(out, (0..(1u32 << min)).map(|v| v as u8).collect::<Vec<_>>());
    }
}

#[test]
fn lzw_kwkwk_and_clear() {
    let mut d = decoder();
    let mut st = Stream::new(2);
    st.clear();
    st.code(1);
    st.code(6);
    st.eoi();
    let data = st.pack(255);
    assert_eq!(d.init(&data, 2, 0), LzwResult::Ok);
    assert_eq!(
        decode_all(&mut d, &data),
        (LzwResult::EoiCode, vec![1, 1, 1])
    );

    let mut st = Stream::new(2);
    st.clear();
    st.code(0);
    st.code(1);
    st.clear();
    st.code(3);
    st.code(6);
    st.eoi();
    let data = st.pack(255);
    assert_eq!(d.init(&data, 2, 0), LzwResult::Ok);
    assert_eq!(
        decode_all(&mut d, &data),
        (LzwResult::EoiCode, vec![0, 1, 3, 3, 3])
    );
}

#[test]
fn lzw_bad_codes() {
    let mut d = decoder();

    let mut st = Stream::new(2);
    st.clear();
    st.code(0);
    let width = st.width;
    st.put(7, width);
    let data = st.pack(255);
    assert_eq!(d.init(&data, 2, 0), LzwResult::Ok);
    assert_eq!(decode_all(&mut d, &data), (LzwResult::BadCode, vec![0]));

    let mut st = Stream::new(2);
    st.clear();
    st.eoi();
    let data = st.pack(255);
    assert_eq!(d.init(&data, 2, 0), LzwResult::BadIcode);
    let mut st = Stream::new(2);
    st.clear();
    let width = st.width;
    st.put(6, width);
    let data = st.pack(255);
    assert_eq!(d.init(&data, 2, 0), LzwResult::BadIcode);

    let mut st = Stream::new(2);
    st.clear();
    st.clear();
    st.clear();
    st.code(2);
    st.eoi();
    let data = st.pack(255);
    assert_eq!(d.init(&data, 2, 0), LzwResult::Ok);
    assert_eq!(decode_all(&mut d, &data), (LzwResult::EoiCode, vec![2]));

    let mut st = Stream::new(11);
    st.clear();
    st.code(2047);
    st.eoi();
    let data = st.pack(255);
    assert_eq!(d.init(&data, 11, 0), LzwResult::Ok);
    assert_eq!(d.init(&data, 12, 0), LzwResult::BadIcode);
    assert_eq!(d.init(&data, 255, 0), LzwResult::BadIcode);
}

#[test]
fn lzw_end_of_data() {
    let mut d = decoder();
    let mut st = Stream::new(2);
    st.clear();
    st.code(0);
    st.code(1);
    let data = st.pack(255);
    assert_eq!(d.init(&data, 2, 0), LzwResult::Ok);
    assert_eq!(
        decode_all(&mut d, &data),
        (LzwResult::OkEod, vec![0, 1, 0, 0])
    );

    let short = &data[..data.len() - 1];
    assert_eq!(d.init(short, 2, 0), LzwResult::Ok);
    assert_eq!(
        decode_all(&mut d, short),
        (LzwResult::NoData, vec![0, 1, 0, 0])
    );

    assert_eq!(d.init(&[0], 2, 0), LzwResult::OkEod);
    assert_eq!(d.init(&[], 2, 0), LzwResult::NoData);
    assert_eq!(d.init(&[0], 2, 5), LzwResult::NoData);
    assert_eq!(d.init(&[1], 2, 0), LzwResult::NoData);
}

#[test]
fn lzw_codes_across_sub_blocks() {
    let mut d = decoder();
    let mut st = Stream::new(8);
    st.clear();
    for i in 0..4000u32 {
        st.code((i * 7) % 256);
    }
    assert_eq!(st.width, 12);
    st.eoi();
    let expected: Vec<u8> = (0..4000u32).map(|i| ((i * 7) % 256) as u8).collect();
    for size in [255, 1, 2, 3] {
        let data = st.pack(size);
        assert_eq!(d.init(&data, 8, 0), LzwResult::Ok);
        assert_eq!(
            decode_all(&mut d, &data),
            (LzwResult::EoiCode, expected.clone())
        );
    }
}

#[test]
fn lzw_eoi_on_byte_boundary() {
    let mut d = decoder();
    for min in 2..=11u32 {
        let mut st = Stream::new(min);
        st.clear();
        let mut n = 0;
        while !(st.bits() + st.width as usize).is_multiple_of(8) {
            st.code(0);
            n += 1;
        }
        st.eoi();
        let data = st.pack(255);
        let res = d.init(&data, min as u8, 0);
        if n == 0 {
            assert_eq!(res, LzwResult::BadIcode);
            continue;
        }
        assert_eq!(res, LzwResult::Ok);
        assert_eq!(decode_all(&mut d, &data), (LzwResult::EoiCode, vec![0; n]));
    }
}

#[test]
fn lzw_full_table() {
    let mut d = decoder();
    let mut st = Stream::new(8);
    st.clear();
    for i in 0..3839u32 {
        st.code(i % 256);
    }
    assert_eq!(st.table_size, 4096);
    for _ in 0..100 {
        st.code(4095);
    }
    st.eoi();
    let data = st.pack(255);
    assert_eq!(d.init(&data, 8, 0), LzwResult::Ok);
    let (res, out) = decode_all(&mut d, &data);
    assert_eq!(res, LzwResult::EoiCode);
    assert_eq!(out.len(), 3839 + 100 * 2);
    for i in 0..100 {
        assert_eq!(out[3839 + 2 * i], (3837 % 256) as u8);
        assert_eq!(out[3839 + 2 * i + 1], (3838 % 256) as u8);
    }
}

#[test]
fn lzw_output_stack_full() {
    let mut d = decoder();
    let mut st = Stream::new(2);
    st.clear();
    st.code(3);
    let mut total = 1;
    let mut k = 2;
    while total + k <= 5000 {
        let ts = st.table_size;
        st.code(ts);
        total += k;
        k += 1;
    }
    st.eoi();
    let data = st.pack(255);
    assert_eq!(d.init(&data, 2, 0), LzwResult::Ok);
    let (res, n) = d.decode(&data);
    assert_eq!((res, n), (LzwResult::Ok, 4096));
    assert!(d.output().iter().all(|&v| v == 3));
    let (res, n2) = d.decode(&data);
    assert_eq!(res, LzwResult::EoiCode);
    assert!(d.output()[..n2 as usize].iter().all(|&v| v == 3));
    assert_eq!(4096 + n2 as usize, total);
}

fn colours() -> [u32; 256] {
    let mut c = [0; 256];
    c[0] = 0xff00_0000;
    c[1] = 0xff00_00ff;
    c[2] = 0xff00_ff00;
    c[3] = 0xffff_0000;
    c
}

#[test]
fn lzw_decode_map_colours_and_transparency() {
    let mut d = decoder();
    let colours = colours();
    let mut st = Stream::new(2);
    st.clear();
    for v in 0..4 {
        st.code(v);
    }
    st.eoi();
    let data = st.pack(255);

    for t in [256, 0x102, 0xffff_ffff] {
        let mut px = [0xdead_beef_u32; 8];
        assert_eq!(d.init_map(&data, 2, t, 0), LzwResult::Ok);
        assert_eq!(
            d.decode_map(&data, Some(&colours), &mut px[..4]),
            (LzwResult::Ok, 4)
        );
        assert_eq!(&px[..4], &colours[..4]);
        assert_eq!(px[4], 0xdead_beef);
    }

    let mut px = [0xdead_beef_u32; 8];
    assert_eq!(d.init_map(&data, 2, 2, 0), LzwResult::Ok);
    assert_eq!(
        d.decode_map(&data, Some(&colours), &mut px),
        (LzwResult::EoiCode, 4)
    );
    assert_eq!(px[..4], [colours[0], colours[1], 0xdead_beef, colours[3]]);

    assert_eq!(d.init_map(&data, 2, 256, 0), LzwResult::Ok);
    let mut one = [0u32; 1];
    for c in &colours[..4] {
        assert_eq!(
            d.decode_map(&data, Some(&colours), &mut one),
            (LzwResult::Ok, 1)
        );
        assert_eq!(one[0], *c);
    }
    assert_eq!(
        d.decode_map(&data, Some(&colours), &mut one),
        (LzwResult::EoiCode, 0)
    );

    assert_eq!(d.init_map(&data, 2, 256, 0), LzwResult::Ok);
    assert_eq!(
        d.decode_map(&data, Some(&colours), &mut []),
        (LzwResult::Ok, 0)
    );
}

#[test]
fn lzw_decode_map_needs_a_colour_table() {
    let mut d = decoder();
    let mut st = Stream::new(2);
    st.clear();
    st.code(0);
    st.eoi();
    let data = st.pack(255);
    assert_eq!(d.init(&data, 2, 0), LzwResult::Ok);
    let mut px = [0u32; 4];
    assert_eq!(d.decode_map(&data, None, &mut px), (LzwResult::NoColour, 0));
}

#[test]
fn edge_full_table_for_70000_codes() {
    // A deferred clear: the table stays full (table_size 4096) for any
    // number of codes; it never counts on past it.
    let mut d = decoder();
    let mut st = Stream::new(8);
    st.clear();
    for i in 0..3839u32 {
        st.code(i % 256);
    }
    assert_eq!(st.table_size, 4096);
    for i in 0..70_000u32 {
        st.code(i % 256);
    }
    st.eoi();
    let data = st.pack(255);
    assert_eq!(d.init(&data, 8, 0), LzwResult::Ok);
    let (res, out) = decode_all(&mut d, &data);
    assert_eq!(res, LzwResult::EoiCode);
    assert_eq!(out.len(), 3839 + 70_000);
    assert!(
        out[3839..]
            .iter()
            .enumerate()
            .all(|(i, &v)| v == (i % 256) as u8)
    );
}

#[test]
fn edge_minimum_code_sizes_0_and_1() {
    // Not valid GIF (2 to 8), but accepted below 12: the clear code is 1 or
    // 2 and the roots 0 or 0 and 1.
    let mut d = decoder();
    let mut st = Stream::new(1);
    st.clear();
    st.code(1);
    st.code(0);
    st.eoi();
    let data = st.pack(255);
    assert_eq!(d.init(&data, 1, 0), LzwResult::Ok);
    assert_eq!(decode_all(&mut d, &data), (LzwResult::EoiCode, vec![1, 0]));
}

#[test]
fn edge_input_pos_inside_the_data() {
    let mut d = decoder();
    let mut st = Stream::new(2);
    st.clear();
    st.code(3);
    st.eoi();
    let mut data = vec![0xaa, 0xbb];
    data.extend(st.pack(255));
    assert_eq!(d.init(&data, 2, 2), LzwResult::Ok);
    assert_eq!(decode_all(&mut d, &data), (LzwResult::EoiCode, vec![3]));
}

proptest! {
    #[test]
    fn any_input_never_panics(
        data in proptest::collection::vec(any::<u8>(), 0..600),
        min in 0u8..14,
        pos in 0usize..8,
        t in prop_oneof![0u32..300, any::<u32>()],
        lens in proptest::collection::vec(0usize..300, 1..8),
    ) {
        let mut d = decoder();
        let colours = colours();
        if d.init_map(&data, min, t, pos) == LzwResult::Ok {
            let mut out = vec![0u32; 300];
            for len in lens {
                let (res, n) = d.decode_map(&data, Some(&colours), &mut out[..len]);
                prop_assert!(n as usize <= len);
                if res != LzwResult::Ok { break; }
                prop_assert_eq!(n as usize, len);
            }
        }
        if d.init(&data, min, pos) == LzwResult::Ok {
            for _ in 0..4 {
                let (res, n) = d.decode(&data);
                prop_assert!(n as usize <= 4096);
                if res != LzwResult::Ok { break; }
                prop_assert_eq!(n, 4096);
            }
        }
    }

    #[test]
    fn any_root_sequence_round_trips(
        min in 2u32..=8,
        values in proptest::collection::vec(any::<u8>(), 1..3000),
        block in 1usize..=255,
    ) {
        let mut st = Stream::new(min);
        st.clear();
        let mask = (1u32 << min) - 1;
        for v in &values { st.code(u32::from(*v) & mask); }
        st.eoi();
        let data = st.pack(block);
        let mut d = decoder();
        prop_assert_eq!(d.init(&data, min as u8, 0), LzwResult::Ok);
        let (res, out) = decode_all(&mut d, &data);
        prop_assert_eq!(res, LzwResult::EoiCode);
        let expected: Vec<u8> = values.iter().map(|v| (u32::from(*v) & mask) as u8).collect();
        prop_assert_eq!(out, expected);
    }
}

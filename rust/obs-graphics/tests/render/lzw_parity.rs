//! Tier 3: the shims against the original `libobs/graphics/libnsgif/lzw.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/lzw.c`) is the C file, unmodified,
//! with its six functions renamed. Both sides get the same calls through
//! their C signatures, one context each, and are compared after every call:
//! the result, the number of values written, the values (the output stack
//! for `lzw_decode`, the whole pixel buffer for `lzw_decode_map`, so pixels
//! left alone by transparency count too).
//!
//! Runs: the enum values and `LZW_CODE_MAX`; every minimum code size 0 to
//! 13 on emitter streams in sub-blocks of 1 to 4 and 255 bytes; and 20 000
//! random call sequences (init, init_map with and without a colour table,
//! decode, decode_map with output lengths 0 to 300) over random bytes and
//! over emitter streams with random clears, KwKwK codes, bad codes and
//! truncation.
//!
//! The C context is cleared right after creation, so both sides start from
//! the same memory. Without that, decoding on after `LZW_BAD_ICODE` from a
//! clear code inside the data differs: the clear has reset the table, but
//! `prev_code` keeps a code past it, so later records follow `extends` links
//! the C never initialized. With the context filled with 0xFF this test
//! found it at once, and under AddressSanitizer it is a wild read in
//! `lzw__write_fn`. `gif.c` stops at the first error and starts each frame
//! with an init, so libobs never gets there; the Rust context starts zeroed
//! and every table read is bounds-checked.
//!
//! Excluded, the intentional differences (C reads uninitialized memory or
//! dereferences null): a call before the first successful init, and null
//! pointers. Those are tested on the shim alone below.

use core::ffi::c_void;
use core::ptr;

use obs_c_oracle::lzw as c;
use obs_graphics::ffi::lzw::{
    LzwCtx, lzw_context_create, lzw_context_destroy, lzw_decode, lzw_decode_init,
    lzw_decode_init_map, lzw_decode_map,
};
use obs_graphics::lzw::{LZW_CODE_MAX, LzwResult};
use proptest::prelude::*;

use super::lzw::Stream;

#[test]
fn layout() {
    let values = [
        size_of::<LzwResult>(),
        LzwResult::Ok as usize,
        LzwResult::OkEod as usize,
        LzwResult::NoMem as usize,
        LzwResult::NoData as usize,
        LzwResult::EoiCode as usize,
        LzwResult::NoColour as usize,
        LzwResult::BadIcode as usize,
        LzwResult::BadParam as usize,
        LzwResult::BadCode as usize,
        usize::from(LZW_CODE_MAX),
    ];
    for (which, (name, value)) in c::LAYOUT_NAMES.iter().zip(values).enumerate() {
        // SAFETY: a pure C function.
        assert_eq!(unsafe { c::oracle_lzw_layout(which) }, value, "{name}");
    }
}

#[derive(Debug, Clone)]
enum Op {
    Init {
        min: u8,
        pos: usize,
    },
    InitMap {
        min: u8,
        transparency: u32,
        pos: usize,
        table: bool,
    },
    Decode,
    DecodeMap {
        len: usize,
    },
}

/// One context on each side.
struct Pair {
    rust: *mut LzwCtx,
    c: *mut c_void,
}

impl Pair {
    fn new() -> Self {
        let mut rust = ptr::null_mut();
        let mut cc = ptr::null_mut();
        // SAFETY: valid out-pointers.
        unsafe {
            assert_eq!(lzw_context_create(&mut rust), LzwResult::Ok);
            assert_eq!(c::oracle_lzw_context_create(&mut cc), 0);
            // The C context is malloc'd and never cleared; clear it, as the
            // Rust context starts (see the module comment).
            c::oracle_lzw_context_fill(cc, 0);
        }
        Self { rust, c: cc }
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        // SAFETY: from the create functions, not used afterwards.
        unsafe {
            lzw_context_destroy(self.rust);
            c::oracle_lzw_context_destroy(self.c);
        }
    }
}

/// Run `ops` on both sides and compare after each call. The first op must
/// be an init that succeeds on both (before it, the C context is
/// uninitialized); returns false when it does not, and the sequence is
/// skipped.
fn compare(data: &[u8], colours: &[u32; 256], ops: &[Op]) -> bool {
    let p = Pair::new();
    let mut rust_px = vec![0x5a5a_5a5au32; 300];
    let mut c_px = rust_px.clone();
    let mut initialized = false;
    for (i, op) in ops.iter().enumerate() {
        match *op {
            Op::Init { min, pos } => {
                // SAFETY: live contexts; `data` outlives both.
                let (r, cr) = unsafe {
                    (
                        lzw_decode_init(p.rust, min, data.as_ptr(), data.len(), pos),
                        c::oracle_lzw_decode_init(p.c, min, data.as_ptr(), data.len(), pos),
                    )
                };
                assert_eq!(r as i32, cr, "op {i} {op:?}");
                initialized |= r == LzwResult::Ok;
            }
            Op::InitMap {
                min,
                transparency,
                pos,
                table,
            } => {
                let t = if table { colours.as_ptr() } else { ptr::null() };
                // SAFETY: as above; the table has 256 values.
                let (r, cr) = unsafe {
                    (
                        lzw_decode_init_map(
                            p.rust,
                            min,
                            transparency,
                            t,
                            data.as_ptr(),
                            data.len(),
                            pos,
                        ),
                        c::oracle_lzw_decode_init_map(
                            p.c,
                            min,
                            transparency,
                            t,
                            data.as_ptr(),
                            data.len(),
                            pos,
                        ),
                    )
                };
                assert_eq!(r as i32, cr, "op {i} {op:?}");
                initialized |= r == LzwResult::Ok;
            }
            Op::Decode | Op::DecodeMap { .. } if !initialized => return i != 0,
            Op::Decode => {
                let (mut rd, mut cd) = (ptr::null(), ptr::null());
                let (mut rn, mut cn) = (0u32, 0u32);
                // SAFETY: live contexts, valid out-pointers.
                let (r, cr) = unsafe {
                    (
                        lzw_decode(p.rust, &mut rd, &mut rn),
                        c::oracle_lzw_decode(p.c, &mut cd, &mut cn),
                    )
                };
                assert_eq!((r as i32, rn), (cr, cn), "op {i} {op:?}");
                // SAFETY: each side's stack holds `n` values.
                let (rv, cv) = unsafe {
                    (
                        core::slice::from_raw_parts(rd, rn as usize),
                        core::slice::from_raw_parts(cd, cn as usize),
                    )
                };
                assert_eq!(rv, cv, "op {i} {op:?}");
            }
            Op::DecodeMap { len } => {
                let (mut rn, mut cn) = (0u32, 0u32);
                // SAFETY: `len` <= 300 writable values each.
                let (r, cr) = unsafe {
                    (
                        lzw_decode_map(p.rust, rust_px.as_mut_ptr(), len as u32, &mut rn),
                        c::oracle_lzw_decode_map(p.c, c_px.as_mut_ptr(), len as u32, &mut cn),
                    )
                };
                assert_eq!((r as i32, rn), (cr, cn), "op {i} {op:?}");
                assert_eq!(rust_px, c_px, "op {i} {op:?}");
            }
        }
        if i == 0 && !initialized {
            return false;
        }
    }
    true
}

fn colour_table(seed: u32) -> [u32; 256] {
    let mut t = [0u32; 256];
    for (i, v) in t.iter_mut().enumerate() {
        *v = seed
            .wrapping_mul(2_654_435_761)
            .wrapping_add(i as u32 * 0x0101_0101);
    }
    t
}

#[test]
fn every_code_size_and_block_size() {
    let colours = colour_table(7);
    for min in 0u32..=13 {
        let m = min.min(11);
        let mut st = Stream::new(m);
        st.clear();
        let roots = 1u32 << m;
        for i in 0..600u32 {
            if i % 97 == 50 {
                st.clear();
            }
            st.code(i % roots);
        }
        st.eoi();
        for block in [1, 2, 3, 4, 255] {
            let data = st.pack(block);
            let decode: Vec<Op> = core::iter::once(Op::Init {
                min: min as u8,
                pos: 0,
            })
            .chain(core::iter::repeat_n(Op::Decode, 3))
            .collect();
            compare(&data, &colours, &decode);
            let map: Vec<Op> = core::iter::once(Op::InitMap {
                min: min as u8,
                transparency: 3,
                pos: 0,
                table: true,
            })
            .chain((0..40).map(|k| Op::DecodeMap {
                len: [0, 1, 7, 64, 300][k % 5],
            }))
            .collect();
            compare(&data, &colours, &map);
        }
    }
}

/// An emitter stream from a recipe: data codes (roots, the next table
/// code or an existing table code), clears, bad codes; then EOI or not.
fn emitted(min: u32, recipe: &[u16], eoi: bool, block: usize, cut: usize) -> Vec<u8> {
    let mut st = Stream::new(min);
    st.clear();
    let roots = 1u32 << min;
    for &r in recipe {
        match r % 16 {
            0 => st.clear(),
            1 => {
                let ts = st.table_size;
                st.code(ts);
            }
            2 => {
                let w = st.width;
                st.put(u32::from(r) % (1 << w), w);
            }
            3 if st.table_size > roots + 2 => {
                st.code(roots + 2 + u32::from(r) % (st.table_size - roots - 2))
            }
            _ => st.code(u32::from(r) % roots),
        }
    }
    if eoi {
        st.eoi();
    }
    let mut data = st.pack(block);
    data.truncate(data.len().saturating_sub(cut));
    data
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u8..14, 0usize..4).prop_map(|(min, pos)| Op::Init { min, pos }),
        (
            0u8..14,
            prop_oneof![0u32..260, any::<u32>()],
            0usize..4,
            prop::bool::weighted(0.9)
        )
            .prop_map(|(min, transparency, pos, table)| Op::InitMap {
                min,
                transparency,
                pos,
                table
            }),
        Just(Op::Decode),
        (0usize..=300).prop_map(|len| Op::DecodeMap { len }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn random_bytes(
        data in proptest::collection::vec(any::<u8>(), 0..800),
        seed: u32,
        first in (0u8..12, 0usize..3),
        ops in proptest::collection::vec(op(), 1..12),
    ) {
        let colours = colour_table(seed);
        let mut all = vec![Op::Init { min: first.0, pos: first.1 }];
        all.extend(ops);
        compare(&data, &colours, &all);
    }

    #[test]
    fn emitter_streams(
        min in 0u32..=11,
        recipe in proptest::collection::vec(any::<u16>(), 0..2500),
        eoi: bool,
        block in prop_oneof![1usize..=4, 5usize..=255],
        cut in prop_oneof![Just(0usize), 0usize..6],
        seed: u32,
        map: bool,
        ops in proptest::collection::vec(op(), 0..10),
    ) {
        let data = emitted(min, &recipe, eoi, block, cut);
        let colours = colour_table(seed);
        let mut all = if map {
            vec![Op::InitMap { min: min as u8, transparency: seed % 300, pos: 0, table: true }]
        } else {
            vec![Op::Init { min: min as u8, pos: 0 }]
        };
        all.extend(core::iter::repeat_n(if map { Op::DecodeMap { len: 300 } } else { Op::Decode }, 6));
        all.extend(ops);
        compare(&data, &colours, &all);
    }
}

#[test]
fn null_pointers_on_the_shim() {
    // The intentional differences: the C would dereference these.
    let mut ctx = ptr::null_mut();
    let data = [1u8, 0x4c, 0];
    let mut n = 0u32;
    let mut out = ptr::null();
    // SAFETY: null is allowed by the shim's contract; valid pointers
    // otherwise.
    unsafe {
        assert_eq!(lzw_context_create(ptr::null_mut()), LzwResult::BadParam);
        assert_eq!(
            lzw_decode_init(ptr::null_mut(), 2, data.as_ptr(), 3, 0),
            LzwResult::BadParam
        );
        assert_eq!(
            lzw_decode(ptr::null_mut(), &mut out, &mut n),
            LzwResult::BadParam
        );
        assert_eq!(
            lzw_decode_map(ptr::null_mut(), ptr::null_mut(), 0, &mut n),
            LzwResult::BadParam
        );
        lzw_context_destroy(ptr::null_mut());

        assert_eq!(lzw_context_create(&mut ctx), LzwResult::Ok);
        // Before any init: no data, no colour table.
        assert_eq!(lzw_decode(ctx, &mut out, &mut n), LzwResult::NoData);
        assert_eq!(n, 0);
        assert_eq!(
            lzw_decode_map(ctx, [0u32; 4].as_mut_ptr(), 4, &mut n),
            LzwResult::NoColour
        );
        assert_eq!(
            lzw_decode(ctx, ptr::null_mut(), &mut n),
            LzwResult::BadParam
        );
        assert_eq!(
            lzw_decode(ctx, &mut out, ptr::null_mut()),
            LzwResult::BadParam
        );
        assert_eq!(
            lzw_decode_map(ctx, ptr::null_mut(), 4, &mut n),
            LzwResult::BadParam
        );
        assert_eq!(
            lzw_decode_map(ctx, [0u32; 4].as_mut_ptr(), 4, ptr::null_mut()),
            LzwResult::BadParam
        );
        // A null input reads as empty.
        assert_eq!(
            lzw_decode_init(ctx, 2, ptr::null(), 10, 0),
            LzwResult::NoData
        );
        lzw_context_destroy(ctx);
    }
}

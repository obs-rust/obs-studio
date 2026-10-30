//! Tier 3: the shims against the original
//! `libobs/graphics/libnsgif/gif.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/gif.c`) is the C file, unmodified,
//! with its public functions renamed and its calls going to the C oracles
//! of the other libnsgif files, so the C side is the whole of libnsgif in C
//! and the Rust side the whole of it in Rust. Both get the same calls
//! through their C signatures, with the same bitmap callbacks, and are
//! compared after every call: the result, the info, every frame info, the
//! palettes, the redraw area, the delay and the frame of a prepare, the
//! returned bitmap's pixels and the callback counts.
//!
//! Runs: the error and disposal strings; and 3 000 random cases of GIFs
//! from the writer of `render::gif` (one to four frames, every disposal,
//! delays, transparency, rectangles inside and past the screen, interlace,
//! local tables, loop counts, the screen size quirk), mutated at random
//! bytes or cut, and random bytes; each scanned whole or in two to four
//! parts, with or without `nsgif_data_complete`, in all nine bitmap formats,
//! with or without a row span wider than the image and the optional
//! callbacks, then 0 to 24 random calls (prepare, decode of any frame,
//! reset, the palettes, the delay rules). The bitmap callback refuses
//! bitmaps over 16 MiB, as an out-of-memory host does.
//!
//! Excluded, the intentional differences: null pointers (C dereferences
//! null), on the shim alone below; and the local palette colours of a frame
//! a scan stopped in (C decodes the table from past the end of the data when
//! the scan stopped inside it; Rust decodes only the bytes in the data), so
//! for those frames the views keep whether there is a local palette and its
//! size, not its colours.

use core::ffi::{CStr, c_int, c_void};
use core::ptr;

use obs_c_oracle::gif as c;
use obs_graphics::ffi::gif::{
    BitmapCbVt, NsgifCtx, nsgif_create, nsgif_data_complete, nsgif_data_scan, nsgif_destroy,
    nsgif_frame_decode, nsgif_frame_prepare, nsgif_get_frame_info, nsgif_get_info,
    nsgif_global_palette, nsgif_local_palette, nsgif_reset, nsgif_set_frame_delay_behaviour,
    nsgif_str_disposal, nsgif_strerror,
};
use obs_graphics::gif::{FrameInfo, Info};
use obs_graphics::gif_blocks::{MAX_COLOURS, NsgifError, Rect};
use proptest::prelude::*;

use super::gif::Writer;

#[test]
fn strings() {
    for v in (0..12).chain([100, u32::MAX]) {
        // SAFETY: static C strings from both.
        unsafe {
            assert_eq!(
                CStr::from_ptr(nsgif_strerror(v)),
                CStr::from_ptr(c::oracle_nsgif_strerror(v))
            );
            assert_eq!(
                CStr::from_ptr(nsgif_str_disposal(v)),
                CStr::from_ptr(c::oracle_nsgif_str_disposal(v))
            );
        }
    }
}

/// A bitmap of the test callbacks: its pixels, its size and the calls it
/// got.
struct Bitmap {
    width: u32,
    extra: u32,
    pixels: Vec<u32>,
    modified: u32,
    set_opaque: u32,
    last_opaque: bool,
    test_opaque: u32,
}

std::thread_local! {
    static EXTRA: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

/// The largest bitmap the callback makes, in pixels (16 MiB). A mutated
/// screen descriptor can ask for 65535 x 65535 (17 GB, all written), which
/// starved 16 GB CI runners; above this the callback fails as an
/// out-of-memory host does, the same for the C and the Rust side.
const MAX_BITMAP_PIXELS: u64 = 1 << 22;

unsafe extern "C" fn cb_create(width: c_int, height: c_int) -> *mut c_void {
    let extra = EXTRA.with(core::cell::Cell::get);
    let (w, h) = (width.max(0) as u32, height.max(0) as u32);
    if (u64::from(w) + u64::from(extra)) * u64::from(h) > MAX_BITMAP_PIXELS {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(Bitmap {
        width: w,
        extra,
        pixels: vec![0x5A5A_5A5A; ((w + extra) * h) as usize],
        modified: 0,
        set_opaque: 0,
        last_opaque: false,
        test_opaque: 0,
    }))
    .cast()
}

unsafe extern "C" fn cb_destroy(bitmap: *mut c_void) {
    // SAFETY: from cb_create.
    drop(unsafe { Box::from_raw(bitmap.cast::<Bitmap>()) });
}

unsafe extern "C" fn cb_get_buffer(bitmap: *mut c_void) -> *mut u8 {
    // SAFETY: from cb_create.
    unsafe { (*bitmap.cast::<Bitmap>()).pixels.as_mut_ptr().cast() }
}

unsafe extern "C" fn cb_set_opaque(bitmap: *mut c_void, opaque: bool) {
    // SAFETY: from cb_create.
    let b = unsafe { &mut *bitmap.cast::<Bitmap>() };
    b.set_opaque += 1;
    b.last_opaque = opaque;
}

unsafe extern "C" fn cb_test_opaque(bitmap: *mut c_void) -> bool {
    // SAFETY: from cb_create.
    let b = unsafe { &mut *bitmap.cast::<Bitmap>() };
    b.test_opaque += 1;
    b.pixels.first().is_some_and(|p| p >> 24 == 0xFF)
}

unsafe extern "C" fn cb_modified(bitmap: *mut c_void) {
    // SAFETY: from cb_create.
    unsafe { (*bitmap.cast::<Bitmap>()).modified += 1 };
}

unsafe extern "C" fn cb_get_rowspan(bitmap: *mut c_void) -> u32 {
    // SAFETY: from cb_create.
    let b = unsafe { &*bitmap.cast::<Bitmap>() };
    b.width + b.extra
}

fn vt(full: bool) -> BitmapCbVt {
    BitmapCbVt {
        create: Some(cb_create),
        destroy: Some(cb_destroy),
        get_buffer: Some(cb_get_buffer),
        set_opaque: full.then_some(cb_set_opaque as unsafe extern "C" fn(*mut c_void, bool)),
        test_opaque: full.then_some(cb_test_opaque as unsafe extern "C" fn(*mut c_void) -> bool),
        modified: full.then_some(cb_modified as unsafe extern "C" fn(*mut c_void)),
        get_rowspan: full.then_some(cb_get_rowspan as unsafe extern "C" fn(*mut c_void) -> u32),
    }
}

/// What a bitmap shows: its pixels and its call counts.
fn bitmap_state(bitmap: *mut c_void) -> Option<(Vec<u32>, u32, u32, bool, u32)> {
    // SAFETY: null or from cb_create, live.
    unsafe { bitmap.cast::<Bitmap>().as_ref() }.map(|b| {
        (
            b.pixels.clone(),
            b.modified,
            b.set_opaque,
            b.last_opaque,
            b.test_opaque,
        )
    })
}

/// Everything a client can read of a GIF after a call.
#[derive(Debug, PartialEq)]
struct View {
    info: Info,
    frames: Vec<Option<FrameInfo>>,
    global: (Vec<u32>, usize),
    local: Vec<Option<(Vec<u32>, usize)>>,
}

#[derive(Debug, Clone)]
enum Op {
    Prepare,
    Decode(u32),
    Reset,
    Delay(u16, u16),
}

#[derive(Debug, Clone)]
struct Case {
    data: Vec<u8>,
    cuts: Vec<usize>,
    complete: bool,
    fmt: u32,
    full_callbacks: bool,
    extra: u32,
    ops: Vec<Op>,
}

fn writer_gif() -> impl Strategy<Value = Vec<u8>> {
    let frame = (
        proptest::option::of((0u32..8, 0u32..300, proptest::option::of(0u32..6))),
        (0u32..10, 0u32..10, 0u32..10, 0u32..10),
        any::<bool>(),
        proptest::option::of(0u32..3),
        0u32..6,
    );
    (
        prop_oneof![Just((640u32, 480u32)), (0u32..12, 0u32..12)],
        proptest::option::of(0u32..3),
        0u32..8,
        proptest::option::of(0u32..3),
        proptest::collection::vec(frame, 1..5),
        any::<bool>(),
    )
        .prop_map(|((w, h), gct, bg, loops, frames, trailer)| {
            let mut wr = Writer::begin(w, h, gct, bg);
            if let Some(l) = loops {
                wr.loops(l);
            }
            for (gce, (x, y, fw, fh), interlaced, lct, index) in frames {
                if let Some((d, delay, t)) = gce {
                    wr.gce(d, delay, t);
                }
                wr.image(x, y, fw, fh, interlaced, lct, index);
            }
            if trailer { wr.end() } else { wr.gb }
        })
}

fn case() -> impl Strategy<Value = Case> {
    let data = prop_oneof![
        4 => writer_gif(),
        3 => (writer_gif(), proptest::collection::vec((any::<proptest::sample::Index>(), any::<u8>()), 1..4))
            .prop_map(|(mut d, muts)| {
                for (i, b) in muts {
                    let at = i.index(d.len());
                    d[at] = b;
                }
                d
            }),
        2 => (writer_gif(), any::<proptest::sample::Index>()).prop_map(|(mut d, i)| {
            d.truncate(i.index(d.len() + 1));
            d
        }),
        1 => proptest::collection::vec(any::<u8>(), 0..80),
    ];
    let op = prop_oneof![
        3 => Just(Op::Prepare),
        3 => (0u32..6).prop_map(Op::Decode),
        1 => Just(Op::Reset),
        1 => (0u16..12, 0u16..60).prop_map(|(a, b)| Op::Delay(a, b)),
    ];
    (
        data,
        proptest::collection::vec(any::<proptest::sample::Index>(), 0..4),
        any::<bool>(),
        0u32..9,
        any::<bool>(),
        prop_oneof![Just(0u32), 1u32..4],
        proptest::collection::vec(op, 0..24),
    )
        .prop_map(
            |(data, cut_idx, complete, fmt, full_callbacks, extra, ops)| {
                let mut cuts: Vec<usize> =
                    cut_idx.iter().map(|i| i.index(data.len() + 1)).collect();
                cuts.sort_unstable();
                Case {
                    data,
                    cuts,
                    complete,
                    fmt,
                    full_callbacks,
                    extra,
                    ops,
                }
            },
        )
}

/// The maximum local table, in bytes.  The C oracle reads this much from its
/// stored buffer without checking the scan length.
const PALETTE_TAIL_LEN: usize = MAX_COLOURS * 3;
const PALETTE_POISON: u8 = 0xA5;
const PALETTE_PROBE: u8 = 0x5A;

/// The local palette as the views compare it.  `in_data` comes from two C
/// probes over different poison tails, and keeps only entries whose source
/// bytes were inside the scan buffer.
fn local_view(table: [u32; MAX_COLOURS], entries: usize, in_data: usize) -> (Vec<u32>, usize) {
    (table[..entries.min(in_data)].to_vec(), entries)
}

/// Make the C oracle's otherwise lengthless palette read deterministic.
fn set_palette_tail(data: &mut [u8], scan_len: usize, value: u8) {
    if let Some(tail) = data.get_mut(scan_len..scan_len + PALETTE_TAIL_LEN) {
        tail.fill(value);
    }
}

fn prepare_scan_data(data: &mut [u8], input: &[u8], scan_len: usize) {
    if let Some(prefix) = data.get_mut(..input.len()) {
        prefix.copy_from_slice(input);
    }
    set_palette_tail(data, scan_len, PALETTE_POISON);
}

/// Count each C local-palette prefix that did not change when its bytes past
/// `scan_len` were switched to a distinct poison value.
fn c_local_in_data(gif: *mut c_void, data: &mut [u8], scan_len: usize) -> Vec<usize> {
    // The tail is owned padding passed to the scan, so the oracle can read it.
    set_palette_tail(data, scan_len, PALETTE_POISON);
    let info = unsafe { *c::oracle_nsgif_get_info(gif).cast::<Info>() };
    (0..info.frame_count + 2)
        .map(|frame| {
            let mut first = [0u32; MAX_COLOURS];
            let mut entries = 0usize;
            // SAFETY: `gif` and the local output buffers are live.
            if !unsafe {
                c::oracle_nsgif_local_palette(gif, frame, first.as_mut_ptr(), &mut entries)
            } {
                return 0;
            }
            set_palette_tail(data, scan_len, PALETTE_PROBE);
            let mut second = [0u32; MAX_COLOURS];
            let mut second_entries = 0usize;
            // SAFETY: as above; changing only the tail distinguishes past-data reads.
            unsafe {
                c::oracle_nsgif_local_palette(gif, frame, second.as_mut_ptr(), &mut second_entries);
            }
            set_palette_tail(data, scan_len, PALETTE_POISON);
            let comparable = entries.min(second_entries);
            match first
                .iter()
                .zip(second.iter())
                .take(comparable)
                .position(|(a, b)| a != b)
            {
                Some(prefix) => prefix,
                None => comparable,
            }
        })
        .collect()
}

/// The view of the Rust GIF.
fn rust_view(gif: *mut NsgifCtx, in_data: &[usize]) -> View {
    // SAFETY: a live GIF; the out-pointers are local.
    unsafe {
        let info = *nsgif_get_info(gif);
        let n = info.frame_count + 2;
        let frames = (0..n)
            .map(|f| nsgif_get_frame_info(gif, f).as_ref().copied())
            .collect();
        let mut table = [0u32; MAX_COLOURS];
        let mut entries = 0usize;
        nsgif_global_palette(gif, &mut table, &mut entries);
        let global = (table.to_vec(), entries);
        let local = (0..n)
            .map(|f| {
                let mut t = [0u32; MAX_COLOURS];
                let mut e = 0usize;
                nsgif_local_palette(gif, f, &mut t, &mut e)
                    .then(|| local_view(t, e, in_data.get(f as usize).copied().unwrap_or_default()))
            })
            .collect();
        View {
            info,
            frames,
            global,
            local,
        }
    }
}

/// The view of the C GIF.
fn c_view(gif: *mut c_void, in_data: &[usize]) -> View {
    // SAFETY: a live GIF; the C returns pointers to its nsgif_info_t and
    // nsgif_frame_info_t, which have the layout of Info and FrameInfo.
    unsafe {
        let info = *c::oracle_nsgif_get_info(gif).cast::<Info>();
        let n = info.frame_count + 2;
        let frames = (0..n)
            .map(|f| {
                c::oracle_nsgif_get_frame_info(gif, f)
                    .cast::<FrameInfo>()
                    .as_ref()
                    .copied()
            })
            .collect();
        let mut table = [0u32; MAX_COLOURS];
        let mut entries = 0usize;
        c::oracle_nsgif_global_palette(gif, table.as_mut_ptr(), &mut entries);
        let global = (table.to_vec(), entries);
        let local = (0..n)
            .map(|f| {
                let mut t = [0u32; MAX_COLOURS];
                let mut e = 0usize;
                c::oracle_nsgif_local_palette(gif, f, t.as_mut_ptr(), &mut e)
                    .then(|| local_view(t, e, in_data.get(f as usize).copied().unwrap_or_default()))
            })
            .collect();
        View {
            info,
            frames,
            global,
            local,
        }
    }
}

#[test]
fn local_view_excludes_colours_past_scanned_data() {
    let mut table = [0u32; MAX_COLOURS];
    table[..2].copy_from_slice(&[0x1122_3344, 0x5566_7788]);

    assert_eq!(local_view(table, 2, 1), (vec![0x1122_3344], 2),);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(3000)))]

    #[test]
    fn api(case in case()) {
        EXTRA.with(|e| e.set(case.extra));
        let table = vt(case.full_callbacks);
        let (mut rg, mut cg): (*mut NsgifCtx, *mut c_void) = (ptr::null_mut(), ptr::null_mut());
        // SAFETY: every pointer below is live or local; the data outlives
        // both GIFs.
        unsafe {
            prop_assert_eq!(nsgif_create(&table, case.fmt, &mut rg) as i32,
                c::oracle_nsgif_create((&raw const table).cast(), case.fmt, &mut cg));

            let mut data = case.data.clone();
            data.resize(data.len() + PALETTE_TAIL_LEN, PALETTE_POISON);
            let mut lens = case.cuts.clone();
            lens.push(case.data.len());
            for n in lens {
                prepare_scan_data(&mut data, &case.data, n);
                let r = nsgif_data_scan(rg, n, data.as_ptr());
                let cr = c::oracle_nsgif_data_scan(cg, n, data.as_ptr());
                prop_assert_eq!(r as i32, cr, "scan {}", n);
                let in_data = c_local_in_data(cg, &mut data, n);
                prop_assert_eq!(rust_view(rg, &in_data), c_view(cg, &in_data));
            }
            if case.complete {
                nsgif_data_complete(rg);
                c::oracle_nsgif_data_complete(cg);
                let in_data = c_local_in_data(cg, &mut data, case.data.len());
                prop_assert_eq!(rust_view(rg, &in_data), c_view(cg, &in_data));
            }

            for op in &case.ops {
                match *op {
                    Op::Prepare => {
                        let (mut ra, mut rd, mut rf) = (Rect::default(), 0u32, 0u32);
                        let (mut ca, mut cd, mut cf) = (Rect::default(), 0u32, 0u32);
                        let r = nsgif_frame_prepare(rg, &mut ra, &mut rd, &mut rf);
                        let cr = c::oracle_nsgif_frame_prepare(cg, (&raw mut ca).cast(), &mut cd, &mut cf);
                        prop_assert_eq!(r as i32, cr);
                        prop_assert_eq!((ra, rd, rf), (ca, cd, cf));
                    }
                    Op::Decode(f) => {
                        let (mut rb, mut cb) = (ptr::null_mut::<c_void>(), ptr::null_mut::<c_void>());
                        let r = nsgif_frame_decode(rg, f, &mut rb);
                        let cr = c::oracle_nsgif_frame_decode(cg, f, &mut cb);
                        prop_assert_eq!(r as i32, cr, "decode {}", f);
                        prop_assert_eq!(rb.is_null(), cb.is_null());
                        prop_assert_eq!(bitmap_state(rb), bitmap_state(cb));
                    }
                    Op::Reset => {
                        prop_assert_eq!(nsgif_reset(rg) as i32, c::oracle_nsgif_reset(cg));
                    }
                    Op::Delay(min, default) => {
                        nsgif_set_frame_delay_behaviour(rg, min, default);
                        c::oracle_nsgif_set_frame_delay_behaviour(cg, min, default);
                    }
                }
                let in_data = c_local_in_data(cg, &mut data, case.data.len());
                prop_assert_eq!(rust_view(rg, &in_data), c_view(cg, &in_data));
            }
            nsgif_destroy(rg);
            c::oracle_nsgif_destroy(cg);
        }
    }
}

// ---------------------------------------------------------------------------
// The intentional differences, on the shim alone.

#[test]
fn shim_null_pointers() {
    let table = vt(false);
    let mut g: *mut NsgifCtx = ptr::null_mut();
    // SAFETY: every non-null pointer is valid; the nulls are the test.
    unsafe {
        assert_eq!(nsgif_create(ptr::null(), 0, &mut g), NsgifError::Data);
        assert_eq!(nsgif_create(&table, 0, ptr::null_mut()), NsgifError::Data);
        assert_eq!(
            nsgif_data_scan(ptr::null_mut(), 0, ptr::null()),
            NsgifError::Data
        );
        assert_eq!(nsgif_reset(ptr::null_mut()), NsgifError::Data);
        assert!(nsgif_get_info(ptr::null()).is_null());
        assert!(nsgif_get_frame_info(ptr::null(), 0).is_null());
        nsgif_destroy(ptr::null_mut());
        nsgif_data_complete(ptr::null_mut());

        assert_eq!(nsgif_create(&table, 0, &mut g), NsgifError::Ok);
        // A null data pointer is empty data.
        assert_eq!(nsgif_data_scan(g, 10, ptr::null()), NsgifError::EndOfData);
        let mut area = Rect::default();
        let mut d = 0u32;
        assert_eq!(
            nsgif_frame_prepare(g, &mut area, &mut d, ptr::null_mut()),
            NsgifError::Data
        );
        assert_eq!(
            nsgif_frame_decode(g, 0, ptr::null_mut()),
            NsgifError::BadFrame
        );
        assert!(!nsgif_local_palette(g, 0, ptr::null_mut(), &mut 0));
        nsgif_destroy(g);
    }
}

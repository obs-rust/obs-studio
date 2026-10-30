//! Tier 3: the shims against the original
//! `libobs/graphics/libnsgif/gif-blocks.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/gif_blocks.c`) is the C file,
//! unmodified, with its seven functions renamed. Both sides get the same
//! call through their C signatures and are compared after it: the result,
//! the position, and every value they write (the result structs, the loop
//! count, the image size, the whole colour table).
//!
//! Runs: the enum values and the struct layouts; the screen size quirk for
//! every size around the listed ones; and 20 000 random cases per parser
//! over random bytes and over generated blocks (headers, screen
//! descriptors, every extension kind with right and wrong sizes, image
//! descriptors, colour tables), cut at random lengths and read from random
//! positions in the data, with decode on and off.
//!
//! Excluded, the intentional differences (C dereferences null, wraps a
//! length, or writes past the table): null pointers, a position past the
//! data, and colour layouts that reach past the table. Those are tested on
//! the shim alone below.

use core::ptr;

use obs_c_oracle::gif_blocks as c;
use obs_graphics::ffi::gif_blocks::{
    nsgif_blocks_colour_table, nsgif_blocks_colour_table_decode, nsgif_blocks_extensions,
    nsgif_blocks_header, nsgif_blocks_image_descriptor, nsgif_blocks_screen,
    nsgif_blocks_screen_size_quirk,
};
use obs_graphics::gif_blocks::{
    ColourLayout, FrameExt, Image, MAX_COLOURS, NsgifError, Rect, Screen, disposal,
};
use proptest::prelude::*;

#[test]
fn layout() {
    let values = [
        size_of::<NsgifError>(),
        NsgifError::Ok as usize,
        NsgifError::Oom as usize,
        NsgifError::Data as usize,
        NsgifError::BadFrame as usize,
        NsgifError::DataFrame as usize,
        NsgifError::EndOfData as usize,
        NsgifError::DataComplete as usize,
        NsgifError::FrameDisplay as usize,
        NsgifError::AnimationEnd as usize,
        MAX_COLOURS,
        size_of::<ColourLayout>(),
        size_of::<Screen>(),
        core::mem::offset_of!(Screen, height),
        core::mem::offset_of!(Screen, global_palette),
        core::mem::offset_of!(Screen, colour_table_size),
        core::mem::offset_of!(Screen, bg_index),
        core::mem::offset_of!(Screen, aspect_ratio),
        core::mem::offset_of!(Screen, loop_max),
        size_of::<FrameExt>(),
        core::mem::offset_of!(FrameExt, disposal),
        core::mem::offset_of!(FrameExt, transparency),
        core::mem::offset_of!(FrameExt, transparency_index),
        core::mem::offset_of!(FrameExt, redraw_required),
        size_of::<Image>(),
        core::mem::offset_of!(Image, rect),
        core::mem::offset_of!(Image, interlaced),
        size_of::<Rect>(),
        usize::from(disposal::RESTORE_BG),
        usize::from(disposal::RESTORE_PREV),
        usize::from(disposal::RESTORE_QUIRK),
    ];
    for (which, (name, value)) in c::LAYOUT_NAMES.iter().zip(values).enumerate() {
        // SAFETY: a pure C function.
        assert_eq!(
            unsafe { c::oracle_gif_blocks_layout(which) },
            value,
            "{name}"
        );
    }
}

#[test]
fn screen_size_quirk_around_every_listed_size() {
    let listed = [
        (640u32, 480u32),
        (640, 512),
        (800, 600),
        (1024, 768),
        (1280, 1024),
        (1600, 1200),
        (2048, 2048),
        (0, 0),
    ];
    for (w, h) in listed {
        for dw in [0u32, 1, 2] {
            for dh in [0u32, 1, 2] {
                for (w, h) in [
                    (
                        w.wrapping_add(dw).wrapping_sub(1),
                        h.wrapping_add(dh).wrapping_sub(1),
                    ),
                    (w.wrapping_add(dw), h.wrapping_add(dh)),
                ] {
                    let (mut rw, mut rh, mut cw, mut ch) = (w, h, w, h);
                    // SAFETY: valid pointers.
                    unsafe {
                        nsgif_blocks_screen_size_quirk(&mut rw, &mut rh);
                        c::oracle_nsgif_blocks_screen_size_quirk(&mut cw, &mut ch);
                    }
                    assert_eq!((rw, rh), (cw, ch), "{w}x{h}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Generated blocks.

fn sub_blocks(payload: &[u8], max: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in payload.chunks(max.clamp(1, 255)) {
        out.push(chunk.len() as u8);
        out.extend_from_slice(chunk);
    }
    out.push(0);
    out
}

fn extension() -> impl Strategy<Value = Vec<u8>> {
    let gce = (
        any::<u8>(),
        prop_oneof![Just(4u8), any::<u8>()],
        any::<[u8; 4]>(),
        any::<bool>(),
    )
        .prop_map(|(_, size, fields, terminated)| {
            let mut v = vec![0x21, 0xF9, size];
            v.extend_from_slice(&fields);
            if terminated {
                v.push(0);
            }
            v
        });
    let app = (
        prop_oneof![
            Just(*b"NETSCAPE2.0"),
            Just(*b"ANIMEXTS1.0"),
            Just(*b"NETSCAPE2.1"),
            any::<[u8; 11]>()
        ],
        prop_oneof![Just(11u8), any::<u8>()],
        prop_oneof![Just(3u8), any::<u8>()],
        prop_oneof![Just(1u8), any::<u8>()],
        any::<u16>(),
    )
        .prop_map(|(ident, size, sb, id, count)| {
            let mut v = vec![0x21, 0xFF, size];
            v.extend_from_slice(&ident);
            v.extend_from_slice(&[sb, id]);
            v.extend_from_slice(&count.to_le_bytes());
            v.push(0);
            v
        });
    let comment = (proptest::collection::vec(any::<u8>(), 0..300), 1usize..256)
        .prop_map(|(p, max)| [vec![0x21, 0xFE], sub_blocks(&p, max)].concat());
    let other = (
        any::<u8>(),
        any::<u8>(),
        proptest::collection::vec(any::<u8>(), 0..40),
        1usize..20,
    )
        .prop_map(|(label, size, p, max)| {
            let mut v = vec![0x21, label, size % 24];
            v.extend(core::iter::repeat_n(0x11, usize::from(size % 24)));
            v.extend(sub_blocks(&p, max));
            v
        });
    prop_oneof![
        gce,
        app,
        comment,
        other,
        proptest::collection::vec(any::<u8>(), 0..6).prop_map(|mut v| {
            v.insert(0, 0x21);
            v
        })
    ]
}

fn image_descriptor() -> impl Strategy<Value = Vec<u8>> {
    (
        prop_oneof![Just(0x2Cu8), any::<u8>()],
        any::<[u16; 4]>(),
        any::<u8>(),
    )
        .prop_map(|(sep, fields, flags)| {
            let mut v = vec![sep];
            for f in fields {
                v.extend_from_slice(&f.to_le_bytes());
            }
            v.push(flags);
            v
        })
}

fn gif_bytes() -> impl Strategy<Value = Vec<u8>> {
    let header = prop_oneof![
        Just(b"GIF89a".to_vec()),
        Just(b"GIF87a".to_vec()),
        any::<[u8; 3]>().prop_map(|v| [b"GIF".as_slice(), &v].concat()),
        any::<[u8; 6]>().prop_map(|v| v.to_vec()),
    ];
    let structured = (
        header,
        any::<[u8; 7]>(),
        proptest::collection::vec(extension(), 0..5),
        image_descriptor(),
        proptest::collection::vec(any::<u8>(), 0..40),
    )
        .prop_map(|(h, lsd, exts, desc, tail)| {
            [h, lsd.to_vec(), exts.concat(), desc, tail].concat()
        });
    prop_oneof![
        1 => proptest::collection::vec(any::<u8>(), 0..64),
        3 => structured,
    ]
}

/// Data, a cut length and a start position at or before it.
fn data_and_pos() -> impl Strategy<Value = (Vec<u8>, usize)> {
    (
        gif_bytes(),
        any::<proptest::sample::Index>(),
        any::<proptest::sample::Index>(),
    )
        .prop_map(|(mut data, cut, start)| {
            let len = cut.index(data.len() + 1);
            data.truncate(len);
            let pos = start.index(len + 1);
            (data, pos)
        })
}

fn frame_ext() -> impl Strategy<Value = FrameExt> {
    (
        any::<u32>(),
        any::<u8>(),
        any::<bool>(),
        any::<u32>(),
        any::<bool>(),
    )
        .prop_map(
            |(delay, disposal, transparency, transparency_index, redraw_required)| FrameExt {
                delay,
                disposal,
                transparency,
                transparency_index,
                redraw_required,
            },
        )
}

/// The data on the heap with exactly its length, so a read past it would
/// show under a sanitizer, and a valid pointer for an empty slice.
fn ptr_of(data: &[u8]) -> *const u8 {
    if data.is_empty() {
        ptr::NonNull::dangling().as_ptr()
    } else {
        data.as_ptr()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn header_and_screen((data, start) in data_and_pos(), strict in any::<bool>()) {
        let (mut rp, mut cp) = (start, start);
        // SAFETY: data is readable for its length; the positions are valid.
        let (r, cr) = unsafe {
            (
                nsgif_blocks_header(ptr_of(&data), data.len(), &mut rp, strict),
                c::oracle_nsgif_blocks_header(ptr_of(&data), data.len(), &mut cp, strict),
            )
        };
        prop_assert_eq!(r as i32, cr);
        prop_assert_eq!(rp, cp);

        let marker = Screen { width: 0xDEAD, loop_max: -7, ..Screen::default() };
        let (mut rs, mut cs) = (marker, marker);
        let (mut rp, mut cp) = (start, start);
        // SAFETY: as above; Screen has the C layout (`layout`).
        let (r, cr) = unsafe {
            (
                nsgif_blocks_screen(ptr_of(&data), data.len(), &mut rp, &mut rs),
                c::oracle_nsgif_blocks_screen(ptr_of(&data), data.len(), &mut cp, (&raw mut cs).cast()),
            )
        };
        prop_assert_eq!(r as i32, cr);
        prop_assert_eq!(rp, cp);
        prop_assert_eq!(rs, cs);
    }

    #[test]
    fn extensions(
        (data, start) in data_and_pos(),
        decode in any::<bool>(),
        ext in frame_ext(),
        loop_max in any::<i32>(),
    ) {
        let (mut re, mut ce) = (ext, ext);
        let (mut rl, mut cl) = (loop_max, loop_max);
        let (mut rp, mut cp) = (start, start);
        // SAFETY: as above; FrameExt has the C layout and valid bools.
        let (r, cr) = unsafe {
            (
                nsgif_blocks_extensions(ptr_of(&data), data.len(), &mut rp, decode, &mut re, &mut rl),
                c::oracle_nsgif_blocks_extensions(ptr_of(&data), data.len(), &mut cp, decode, (&raw mut ce).cast(), &mut cl),
            )
        };
        prop_assert_eq!(r as i32, cr);
        prop_assert_eq!(rp, cp);
        prop_assert_eq!(re, ce);
        prop_assert_eq!(rl, cl);
    }

    #[test]
    fn image_descriptors(
        (data, start) in data_and_pos(),
        decode in any::<bool>(),
        first in any::<bool>(),
        width in prop_oneof![any::<u32>(), 0u32..70_000],
        height in prop_oneof![any::<u32>(), 0u32..70_000],
    ) {
        let marker = Image { flags: 0x5A5A, rect: Rect { x0: 1, y0: 2, x1: 3, y1: 4 }, interlaced: true };
        let (mut ri, mut ci) = (marker, marker);
        let (mut rw, mut rh, mut cw, mut ch) = (width, height, width, height);
        let (mut rp, mut cp) = (start, start);
        // SAFETY: as above; Image has the C layout.
        let (r, cr) = unsafe {
            (
                nsgif_blocks_image_descriptor(ptr_of(&data), data.len(), &mut rp, decode, first, &mut ri, &mut rw, &mut rh),
                c::oracle_nsgif_blocks_image_descriptor(
                    ptr_of(&data), data.len(), &mut cp, decode, first, (&raw mut ci).cast(), &mut cw, &mut ch,
                ),
            )
        };
        prop_assert_eq!(r as i32, cr);
        prop_assert_eq!(rp, cp);
        prop_assert_eq!(ri, ci);
        prop_assert_eq!((rw, rh), (cw, ch));
    }

    #[test]
    fn colour_tables(
        data in proptest::collection::vec(any::<u8>(), 0..800),
        entries in prop_oneof![(0usize..8).prop_map(|b| 2usize << b), 0usize..=256],
        offsets in any::<[u8; 4]>(),
        decode in any::<bool>(),
        plain in any::<bool>(),
        fill in any::<u32>(),
    ) {
        // Offsets 0 to 3, or up to 7 while the last entry is not the
        // table's last: the C writes past the table otherwise.
        let limit = if entries < MAX_COLOURS { 8 } else { 4 };
        let [r, g, b, a] = offsets.map(|o| o % limit);
        let layout = ColourLayout { r, g, b, a };
        let (mut rt, mut ct) = ([fill; MAX_COLOURS], [fill; MAX_COLOURS]);

        if plain {
            // The plain decode checks no length: give it enough data.
            let mut data = data.clone();
            if data.len() < entries * 3 {
                data.resize(entries * 3, 0x77);
            }
            // SAFETY: tables of NSGIF_MAX_COLOURS; data holds entries triplets.
            unsafe {
                nsgif_blocks_colour_table_decode(&mut rt, &layout, entries, ptr_of(&data));
                c::oracle_nsgif_blocks_colour_table_decode(ct.as_mut_ptr(), (&raw const layout).cast(), entries, ptr_of(&data));
            }
        } else {
            let (mut ru, mut cu) = (usize::MAX, usize::MAX);
            // SAFETY: as above, with the data length passed.
            let (res, cres) = unsafe {
                (
                    nsgif_blocks_colour_table(&mut rt, &layout, entries, ptr_of(&data), data.len(), &mut ru, decode),
                    c::oracle_nsgif_blocks_colour_table(
                        ct.as_mut_ptr(), (&raw const layout).cast(), entries, ptr_of(&data), data.len(), &mut cu, decode,
                    ),
                )
            };
            prop_assert_eq!(res as i32, cres);
            prop_assert_eq!(ru, cu);
        }
        prop_assert_eq!(rt, ct);
    }
}

// ---------------------------------------------------------------------------
// The intentional differences, on the shim alone.

#[test]
fn shim_null_pointers_write_nothing() {
    let data = *b"GIF89a\x10\x00\x10\x00\x80\x00\x00";
    let mut pos = 0usize;
    let mut ext = FrameExt::default();
    let mut image = Image::default();
    let (mut h, mut loop_max, mut used) = (0u32, 0i32, 0usize);
    let mut table = [0u32; MAX_COLOURS];
    let layout = ColourLayout {
        r: 0,
        g: 1,
        b: 2,
        a: 3,
    };
    // SAFETY: every non-null pointer is valid; the nulls are the test.
    unsafe {
        assert_eq!(
            nsgif_blocks_header(data.as_ptr(), data.len(), ptr::null_mut(), true),
            NsgifError::Data
        );
        assert_eq!(
            nsgif_blocks_screen(data.as_ptr(), data.len(), &mut pos, ptr::null_mut()),
            NsgifError::Data
        );
        assert_eq!(
            nsgif_blocks_extensions(
                data.as_ptr(),
                data.len(),
                &mut pos,
                true,
                ptr::null_mut(),
                &mut loop_max
            ),
            NsgifError::Data
        );
        assert_eq!(
            nsgif_blocks_extensions(
                data.as_ptr(),
                data.len(),
                &mut pos,
                true,
                &mut ext,
                ptr::null_mut()
            ),
            NsgifError::Data
        );
        assert_eq!(
            nsgif_blocks_image_descriptor(
                data.as_ptr(),
                data.len(),
                &mut pos,
                true,
                true,
                &mut image,
                ptr::null_mut(),
                &mut h
            ),
            NsgifError::Data
        );
        assert_eq!(
            nsgif_blocks_colour_table(
                &mut table,
                ptr::null(),
                2,
                data.as_ptr(),
                data.len(),
                &mut used,
                true
            ),
            NsgifError::Data
        );
        nsgif_blocks_colour_table_decode(ptr::null_mut(), &layout, 2, data.as_ptr());
        nsgif_blocks_screen_size_quirk(ptr::null_mut(), &mut h);
        // A null data pointer reads as empty.
        assert_eq!(
            nsgif_blocks_header(ptr::null(), 99, &mut pos, true),
            NsgifError::EndOfData
        );
    }
    assert_eq!(pos, 0);
    assert_eq!((h, used), (0, 0));
    assert_eq!(table, [0u32; MAX_COLOURS]);
}

#[test]
fn shim_position_past_the_data() {
    let data = *b"GIF89a";
    let mut pos = 9usize;
    // SAFETY: valid pointers.
    unsafe {
        assert_eq!(
            nsgif_blocks_header(data.as_ptr(), data.len(), &mut pos, false),
            NsgifError::EndOfData
        );
        let mut ext = FrameExt::default();
        let mut loop_max = 1;
        assert_eq!(
            nsgif_blocks_extensions(
                data.as_ptr(),
                data.len(),
                &mut pos,
                true,
                &mut ext,
                &mut loop_max
            ),
            NsgifError::Ok
        );
    }
    assert_eq!(pos, data.len());
}

//! Tier 1: the safe core of `libobs/graphics/libnsgif/gif-blocks.c.h`.
//!
//! The `blocks_*` cases mirror the block parser cases of
//! `test/cmocka/test_nsgif_decode.c` one to one, with the same names,
//! bytes and expected values. The cases after those pin the edges of the
//! safe API, and proptests check that no input panics and the invariants.

use obs_graphics::gif_blocks::{
    self, ColourLayout, FrameExt, Image, MAX_COLOURS, NsgifError, Screen, disposal,
};
use proptest::prelude::*;

const RGBA: ColourLayout = ColourLayout {
    r: 0,
    g: 1,
    b: 2,
    a: 3,
};
const BGRA: ColourLayout = ColourLayout {
    b: 0,
    g: 1,
    r: 2,
    a: 3,
};

/// Frame fields before an extension: values no extension writes.
fn ext_unset() -> FrameExt {
    FrameExt {
        delay: 77,
        disposal: 9,
        transparency: false,
        transparency_index: 0xFFFF_FFFF,
        redraw_required: false,
    }
}

fn bytes(table: &[u32; MAX_COLOURS], entry: usize) -> [u8; 4] {
    table[entry].to_ne_bytes()
}

#[test]
fn blocks_header() {
    let mut pos = 0;
    assert_eq!(
        gif_blocks::header(b"GIF89a", &mut pos, true),
        NsgifError::Ok
    );
    assert_eq!(pos, 6);
    pos = 0;
    assert_eq!(
        gif_blocks::header(b"GIF87a", &mut pos, true),
        NsgifError::Ok
    );
    assert_eq!(pos, 6);

    // Kept, characterized, not endorsed: lenient takes any version.
    pos = 0;
    assert_eq!(
        gif_blocks::header(b"GIF90a", &mut pos, true),
        NsgifError::Data
    );
    assert_eq!(pos, 0);
    assert_eq!(
        gif_blocks::header(b"GIF90a", &mut pos, false),
        NsgifError::Ok
    );
    assert_eq!(pos, 6);

    pos = 0;
    assert_eq!(
        gif_blocks::header(b"GIX89a", &mut pos, false),
        NsgifError::Data
    );
    assert_eq!(
        gif_blocks::header(b"GIF89", &mut pos, false),
        NsgifError::EndOfData
    );
    assert_eq!(pos, 0);

    pos = 3;
    assert_eq!(
        gif_blocks::header(b"xyzGIF89a", &mut pos, true),
        NsgifError::Ok
    );
    assert_eq!(pos, 9);
    pos = 4;
    assert_eq!(
        gif_blocks::header(b"xyzGIF89a", &mut pos, true),
        NsgifError::EndOfData
    );
    assert_eq!(pos, 4);
}

#[test]
fn blocks_screen() {
    let lsd = [
        b'G', b'I', b'F', b'8', b'9', b'a', 0x34, 0x12, 0x02, 0x01, 0xF5, 0x07, 0x31,
    ];
    let mut out = Screen::default();
    let mut pos = 6;
    assert_eq!(gif_blocks::screen(&lsd, &mut pos, &mut out), NsgifError::Ok);
    assert_eq!(pos, 13);
    assert_eq!(
        out,
        Screen {
            width: 0x1234,
            height: 0x0102,
            global_palette: true,
            colour_table_size: 64,
            bg_index: 7,
            aspect_ratio: 0x31,
            loop_max: 1,
        }
    );

    let no_table = [0x10, 0x00, 0x08, 0x00, 0x07, 0xFF, 0x00];
    pos = 0;
    assert_eq!(
        gif_blocks::screen(&no_table, &mut pos, &mut out),
        NsgifError::Ok
    );
    assert!(!out.global_palette);
    assert_eq!(out.colour_table_size, 256);
    assert_eq!(out.bg_index, 255);

    let mut untouched = Screen {
        width: 0xABAB_ABAB,
        ..Screen::default()
    };
    pos = 0;
    assert_eq!(
        gif_blocks::screen(&no_table[..6], &mut pos, &mut untouched),
        NsgifError::EndOfData
    );
    assert_eq!(pos, 0);
    assert_eq!(untouched.width, 0xABAB_ABAB);
}

#[test]
fn blocks_screen_size_quirk() {
    let cases: [(u32, u32, u32, u32); 16] = [
        (640, 480, 1, 1),
        (640, 512, 1, 1),
        (800, 600, 1, 1),
        (1024, 768, 1, 1),
        (1280, 1024, 1, 1),
        (1600, 1200, 1, 1),
        (0, 5, 1, 1),
        (5, 0, 1, 1),
        (2049, 5, 1, 1),
        (5, 2049, 1, 1),
        (65535, 65535, 1, 1),
        (2048, 2048, 2048, 2048),
        (480, 640, 480, 640),
        (640, 481, 640, 481),
        (1920, 1080, 1920, 1080),
        (1, 1, 1, 1),
    ];
    for (w, h, out_w, out_h) in cases {
        let (mut width, mut height) = (w, h);
        gif_blocks::screen_size_quirk(&mut width, &mut height);
        assert_eq!((width, height), (out_w, out_h), "{w}x{h}");
    }
}

#[test]
fn blocks_colour_table() {
    let rgb: Vec<u8> = (0..3 * 256usize).map(|i| (i * 7 + 1) as u8).collect();
    let mut table = [0u32; MAX_COLOURS];
    let mut used = 0;

    for bits in 0..8 {
        let entries = 2usize << bits;
        table.fill(0x5A5A_5A5A);
        used = 0;
        assert_eq!(
            gif_blocks::colour_table(
                &mut table,
                RGBA,
                entries,
                &rgb[..entries * 3],
                &mut used,
                true
            ),
            NsgifError::Ok
        );
        assert_eq!(used, entries * 3);
        for e in 0..entries {
            assert_eq!(
                bytes(&table, e),
                [rgb[3 * e], rgb[3 * e + 1], rgb[3 * e + 2], 0xFF]
            );
        }
        if entries < MAX_COLOURS {
            assert_eq!(table[entries], 0x5A5A_5A5A);
        }

        used = 99;
        assert_eq!(
            gif_blocks::colour_table(
                &mut table,
                RGBA,
                entries,
                &rgb[..entries * 3 - 1],
                &mut used,
                true
            ),
            NsgifError::EndOfData
        );
        assert_eq!(used, 99);
    }

    assert_eq!(
        gif_blocks::colour_table(&mut table, BGRA, 2, &rgb[..6], &mut used, true),
        NsgifError::Ok
    );
    assert_eq!(bytes(&table, 1), [rgb[5], rgb[4], rgb[3], 0xFF]);

    table.fill(0x5A5A_5A5A);
    assert_eq!(
        gif_blocks::colour_table(&mut table, RGBA, 4, &rgb[..12], &mut used, false),
        NsgifError::Ok
    );
    assert_eq!(used, 12);
    assert_eq!(table[0], 0x5A5A_5A5A);
    assert_eq!(
        gif_blocks::colour_table(&mut table, RGBA, 4, &rgb[..11], &mut used, false),
        NsgifError::EndOfData
    );

    gif_blocks::colour_table_decode(&mut table, RGBA, 3, &rgb);
    assert_eq!(bytes(&table, 2)[0], rgb[6]);
    assert_eq!(bytes(&table, 2)[3], 0xFF);
    assert_eq!(table[3], 0x5A5A_5A5A);
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn blocks_graphic_control() {
    for d in 0u8..8 {
        let gce = [
            0x21,
            0xF9,
            0x04,
            d << 2 | 0xE0,
            0x34,
            0x12,
            0x05,
            0x00,
            0x2C,
        ];
        let mut ext = ext_unset();
        let mut loop_max = 1;
        let mut pos = 0;
        assert_eq!(
            gif_blocks::extensions(&gce, &mut pos, true, &mut ext, &mut loop_max),
            NsgifError::Ok
        );
        assert_eq!(pos, 8);
        assert_eq!(ext.delay, 0x1234);
        let expect = if d == disposal::RESTORE_QUIRK {
            disposal::RESTORE_PREV
        } else {
            d
        };
        assert_eq!(ext.disposal, expect);
        assert_eq!(
            ext.redraw_required,
            expect == disposal::RESTORE_BG || expect == disposal::RESTORE_PREV
        );
        assert!(!ext.transparency);
        assert_eq!(ext.transparency_index, 0xFFFF_FFFF);
        assert_eq!(loop_max, 1);
    }

    let two = [
        0x21, 0xF9, 0x04, 0x09, 0x00, 0x00, 0x07, 0x00, 0x21, 0xF9, 0x04, 0x00, 0x02, 0x00, 0x09,
        0x00,
    ];
    let mut ext = ext_unset();
    let mut loop_max = 1;
    let mut pos = 0;
    assert_eq!(
        gif_blocks::extensions(&two, &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::Ok
    );
    assert_eq!(pos, two.len());
    assert_eq!(ext.delay, 2);
    assert_eq!(ext.disposal, 0);
    assert!(!ext.redraw_required);
    assert!(ext.transparency);
    assert_eq!(ext.transparency_index, 7);

    ext = ext_unset();
    pos = 0;
    assert_eq!(
        gif_blocks::extensions(&two, &mut pos, false, &mut ext, &mut loop_max),
        NsgifError::Ok
    );
    assert_eq!(pos, two.len());
    assert_eq!(ext, ext_unset());

    // Kept, characterized, not endorsed: the size byte is not checked.
    let size0 = [0x21, 0xF9, 0x00, 0x05, 0x0A, 0x00, 0x07, 0x00];
    ext = ext_unset();
    pos = 0;
    assert_eq!(
        gif_blocks::extensions(&size0, &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::EndOfData
    );
    assert_eq!(pos, 0);
    assert_eq!(ext.delay, 10);
    assert_eq!(ext.disposal, 1);
    assert!(ext.transparency);
    assert_eq!(ext.transparency_index, 7);
}

#[test]
fn blocks_graphic_control_short() {
    let gce = [0x21, 0xF9, 0x04, 0x05, 0x0A];
    let mut ext = ext_unset();
    let mut loop_max = 1;
    for len in 1..=gce.len() {
        let mut pos = 0;
        assert_eq!(
            gif_blocks::extensions(&gce[..len], &mut pos, true, &mut ext, &mut loop_max),
            NsgifError::EndOfData
        );
        assert_eq!(pos, 0);
        assert_eq!(ext, ext_unset());
    }
    // Without decode only the size byte is needed; then a skip past the end
    // is not an error (kept, characterized, not endorsed).
    for len in 1..=gce.len() {
        let mut pos = 0;
        assert_eq!(
            gif_blocks::extensions(&gce[..len], &mut pos, false, &mut ext, &mut loop_max),
            if len < 3 {
                NsgifError::EndOfData
            } else {
                NsgifError::Ok
            }
        );
        assert_eq!(pos, if len < 3 { 0 } else { len });
    }

    let later = [
        0x21, 0xF9, 0x04, 0x01, 0x03, 0x00, 0x02, 0x00, 0x21, 0xF9, 0x04,
    ];
    let mut pos = 0;
    assert_eq!(
        gif_blocks::extensions(&later, &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::EndOfData
    );
    assert_eq!(pos, 0);
    assert_eq!(ext.delay, 3);
    assert!(ext.transparency);
    assert_eq!(ext.transparency_index, 2);
}

fn app_ext(ident: &[u8; 11], value: u16) -> [u8; 19] {
    let mut app = [0u8; 19];
    app[..3].copy_from_slice(&[0x21, 0xFF, 0x0B]);
    app[3..14].copy_from_slice(ident);
    app[14] = 0x03;
    app[15] = 0x01;
    app[16..18].copy_from_slice(&value.to_le_bytes());
    app
}

#[test]
fn blocks_loop_count() {
    let idents: [&[u8; 11]; 4] = [
        b"NETSCAPE2.0",
        b"ANIMEXTS1.0",
        b"NETSCAPE2.1",
        b"XMP DataXMP",
    ];
    let counts: [(u16, i32); 4] = [(0, 0), (1, 2), (5, 6), (0xFFFF, 0x10000)];
    for (i, ident) in idents.iter().enumerate() {
        for (value, expect) in counts {
            let app = app_ext(ident, value);
            let mut ext = ext_unset();
            let mut loop_max = 1;
            let mut pos = 0;
            assert_eq!(
                gif_blocks::extensions(&app, &mut pos, true, &mut ext, &mut loop_max),
                NsgifError::Ok
            );
            assert_eq!(pos, app.len());
            assert_eq!(loop_max, if i < 2 { expect } else { 1 });
            assert_eq!(ext, ext_unset());

            loop_max = 1;
            pos = 0;
            assert_eq!(
                gif_blocks::extensions(&app, &mut pos, false, &mut ext, &mut loop_max),
                NsgifError::Ok
            );
            assert_eq!(loop_max, 1);
        }
    }

    let mut app = app_ext(b"NETSCAPE2.0", 5);
    let mut ext = ext_unset();
    let mut loop_max = 1;
    let mut pos = 0;
    app[15] = 0x02;
    assert_eq!(
        gif_blocks::extensions(&app, &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::Ok
    );
    assert_eq!(loop_max, 1);
    app[15] = 0x01;
    app[14] = 0x04;
    pos = 0;
    assert_eq!(
        gif_blocks::extensions(&app, &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::EndOfData
    );
    assert_eq!(loop_max, 1);

    pos = 0;
    assert_eq!(
        gif_blocks::extensions(&app[..17], &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::EndOfData
    );
    assert_eq!(pos, 0);
}

#[test]
fn blocks_skip_extensions() {
    let data = [
        0x21, 0xFE, 0x03, b'a', b'b', b'c', 0x02, b'd', b'e', 0x00, // comment
        0x21, 0x01, 0x0C, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0x01, b'x',
        0x00, // plain text
        0x21, 0x99, 0x02, 0xAA, 0xBB, 0x00, // unknown
        0x21, 0xFE, 0x00, // empty comment
        0x2C, 0x00,
    ];
    let mut ext = ext_unset();
    let mut loop_max = 1;
    let mut pos = 0;
    assert_eq!(
        gif_blocks::extensions(&data, &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::Ok
    );
    assert_eq!(pos, 37);
    assert_eq!(ext, ext_unset());

    pos = 37;
    assert_eq!(
        gif_blocks::extensions(&data, &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::Ok
    );
    assert_eq!(pos, 37);
    pos = 0;
    assert_eq!(
        gif_blocks::extensions(&[], &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::Ok
    );
    assert_eq!(pos, 0);

    pos = 0;
    assert_eq!(
        gif_blocks::extensions(&data[..1], &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::EndOfData
    );
    assert_eq!(pos, 0);

    let runs_off = [0x21, 0xFE, 0x05, b'a', b'b'];
    pos = 0;
    assert_eq!(
        gif_blocks::extensions(&runs_off, &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::EndOfData
    );
    assert_eq!(pos, 0);

    // Kept, characterized, not endorsed: a size past the end is not an
    // error.
    pos = 0;
    assert_eq!(
        gif_blocks::extensions(&[0x21, 0x99, 0xFF], &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::Ok
    );
    assert_eq!(pos, 3);
    pos = 0;
    assert_eq!(
        gif_blocks::extensions(
            &[0x21, 0x99, 0x01, 0x00],
            &mut pos,
            true,
            &mut ext,
            &mut loop_max
        ),
        NsgifError::Ok
    );
    assert_eq!(pos, 4);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "scenario table: one block per ported step"
)]
fn blocks_image_descriptor() {
    let desc = [
        0x00, 0x2C, 0x05, 0x00, 0x00, 0x00, 0x14, 0x00, 0x05, 0x00, 0xC3, 0x00,
    ];
    let mut image = Image::default();
    let (mut width, mut height) = (10, 10);
    let mut pos = 1;
    assert_eq!(
        gif_blocks::image_descriptor(
            &desc,
            &mut pos,
            true,
            true,
            &mut image,
            &mut width,
            &mut height
        ),
        NsgifError::Ok
    );
    assert_eq!(pos, 11);
    assert_eq!(image.flags, 0xC3);
    assert!(image.interlaced);
    assert_eq!(
        (image.rect.x0, image.rect.y0, image.rect.x1, image.rect.y1),
        (5, 0, 25, 5)
    );
    assert_eq!((width, height), (25, 10));

    width = 10;
    pos = 1;
    assert_eq!(
        gif_blocks::image_descriptor(
            &desc,
            &mut pos,
            true,
            false,
            &mut image,
            &mut width,
            &mut height
        ),
        NsgifError::Ok
    );
    assert_eq!(width, 10);

    let far = [0x2C, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
    pos = 0;
    assert_eq!(
        gif_blocks::image_descriptor(
            &far,
            &mut pos,
            true,
            true,
            &mut image,
            &mut width,
            &mut height
        ),
        NsgifError::Ok
    );
    assert_eq!((image.rect.x1, image.rect.y1), (131_070, 131_070));
    assert!(!image.interlaced);
    assert_eq!((width, height), (131_070, 131_070));

    let bad = [0x2D, 0, 0, 0, 0, 1, 0, 1, 0, 0];
    let untouched = Image {
        flags: 0x5A5A_5A5A,
        ..Image::default()
    };
    image = untouched;
    pos = 0;
    assert_eq!(
        gif_blocks::image_descriptor(
            &bad,
            &mut pos,
            true,
            true,
            &mut image,
            &mut width,
            &mut height
        ),
        NsgifError::DataFrame
    );
    assert_eq!(pos, 0);
    assert_eq!(image, untouched);
    assert_eq!(
        gif_blocks::image_descriptor(
            &bad[..9],
            &mut pos,
            true,
            true,
            &mut image,
            &mut width,
            &mut height
        ),
        NsgifError::EndOfData
    );

    // Kept, characterized, not endorsed: no separator check without
    // decode.
    width = 3;
    pos = 0;
    assert_eq!(
        gif_blocks::image_descriptor(
            &bad,
            &mut pos,
            false,
            true,
            &mut image,
            &mut width,
            &mut height
        ),
        NsgifError::Ok
    );
    assert_eq!(pos, 10);
    assert_eq!(width, 3);
    assert_eq!(image, untouched);
}

// ---------------------------------------------------------------------------
// Edges of the safe API.

#[test]
fn edge_positions_past_the_data_read_nothing() {
    // The C computes length minus position in size_t and wraps; gif.c never
    // passes a position past the data. Here it is the end of the data.
    let data = *b"GIF89a";
    let mut pos = 7;
    assert_eq!(
        gif_blocks::header(&data, &mut pos, false),
        NsgifError::EndOfData
    );
    let mut screen = Screen::default();
    assert_eq!(
        gif_blocks::screen(&data, &mut pos, &mut screen),
        NsgifError::EndOfData
    );
    let mut image = Image::default();
    let (mut w, mut h) = (0, 0);
    assert_eq!(
        gif_blocks::image_descriptor(&data, &mut pos, true, true, &mut image, &mut w, &mut h),
        NsgifError::EndOfData
    );
    assert_eq!(pos, 7);
    let (mut ext, mut loop_max) = (ext_unset(), 1);
    assert_eq!(
        gif_blocks::extensions(&data, &mut pos, true, &mut ext, &mut loop_max),
        NsgifError::Ok
    );
    assert_eq!(pos, 6);
}

#[test]
fn edge_colour_layout_offsets_past_the_entry() {
    // An offset of 4 to 7 writes into the next entry, as in the C; past the
    // table nothing is written.
    let mut table = [0u32; MAX_COLOURS];
    let layout = ColourLayout {
        r: 4,
        g: 5,
        b: 6,
        a: 7,
    };
    gif_blocks::colour_table_decode(&mut table, layout, 2, &[1, 2, 3, 4, 5, 6]);
    assert_eq!(table[0], 0);
    assert_eq!(bytes(&table, 1), [1, 2, 3, 0xFF]);
    assert_eq!(bytes(&table, 2), [4, 5, 6, 0xFF]);

    let mut last = [0u32; MAX_COLOURS];
    gif_blocks::colour_table_decode(&mut last, layout, 256, &[9u8; 768]);
    assert_eq!(bytes(&last, 255), [9, 9, 9, 0xFF]);
}

#[test]
fn edge_colour_table_decode_stops_at_the_data() {
    let mut table = [0x5A5A_5A5Au32; MAX_COLOURS];
    gif_blocks::colour_table_decode(&mut table, RGBA, 4, &[1, 2, 3, 4, 5]);
    assert_eq!(bytes(&table, 0), [1, 2, 3, 0xFF]);
    assert_eq!(table[1], 0x5A5A_5A5A);
}

// ---------------------------------------------------------------------------
// Properties.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn prop_no_panic_and_positions_in_the_data(
        data in proptest::collection::vec(any::<u8>(), 0..300),
        start in 0usize..310,
        decode in any::<bool>(),
        strict in any::<bool>(),
        first in any::<bool>(),
    ) {
        let start = start.min(data.len());

        let mut pos = start;
        if gif_blocks::header(&data, &mut pos, strict) == NsgifError::Ok {
            prop_assert_eq!(pos, start + 6);
        } else {
            prop_assert_eq!(pos, start);
        }

        let mut pos = start;
        let mut screen = Screen::default();
        if gif_blocks::screen(&data, &mut pos, &mut screen) == NsgifError::Ok {
            prop_assert_eq!(pos, start + 7);
            prop_assert!(screen.colour_table_size >= 2 && screen.colour_table_size <= 256);
            prop_assert_eq!(screen.loop_max, 1);
        }

        let mut pos = start;
        let (mut ext, mut loop_max) = (ext_unset(), 1);
        match gif_blocks::extensions(&data, &mut pos, decode, &mut ext, &mut loop_max) {
            NsgifError::Ok => prop_assert!(pos >= start && pos <= data.len()),
            other => {
                prop_assert_eq!(other, NsgifError::EndOfData);
                prop_assert_eq!(pos, start);
            }
        }
        if !decode {
            prop_assert_eq!(ext, ext_unset());
            prop_assert_eq!(loop_max, 1);
        }
        prop_assert!(ext.disposal != disposal::RESTORE_QUIRK);

        let mut pos = start;
        let mut image = Image::default();
        let (mut w, mut h) = (0, 0);
        if gif_blocks::image_descriptor(&data, &mut pos, decode, first, &mut image, &mut w, &mut h) == NsgifError::Ok {
            prop_assert_eq!(pos, start + 10);
        } else {
            prop_assert_eq!(pos, start);
        }
    }

    #[test]
    fn prop_colour_table_length(
        data in proptest::collection::vec(any::<u8>(), 0..800),
        bits in 0usize..8,
        decode in any::<bool>(),
    ) {
        let entries = 2usize << bits;
        let mut table = [0u32; MAX_COLOURS];
        let mut used = 0;
        let res = gif_blocks::colour_table(&mut table, RGBA, entries, &data, &mut used, decode);
        if data.len() >= entries * 3 {
            prop_assert_eq!(res, NsgifError::Ok);
            prop_assert_eq!(used, entries * 3);
            if decode {
                for e in 0..entries {
                    prop_assert_eq!(bytes(&table, e)[3], 0xFF);
                }
            }
        } else {
            prop_assert_eq!(res, NsgifError::EndOfData);
            prop_assert_eq!(used, 0);
        }
    }
}

//! Tier 1: the safe core of `libobs/graphics/libnsgif/gif.c.h`.
//!
//! The `gif_*` cases mirror the state machine cases of
//! `test/cmocka/test_nsgif_decode.c` one to one, with the same GIF writer,
//! names and expected values. The cases after those pin the edges of the
//! safe API, and proptests check that no data and no call order panics.

// Index loops mirror the cmocka cases' positions one to one.
#![allow(clippy::needless_range_loop)]

use obs_graphics::gif::{self, BitmapFmt, Bitmaps, Gif, INFINITE};
use obs_graphics::gif_blocks::{MAX_COLOURS, NsgifError, disposal};
use proptest::prelude::*;

use super::lzw::Stream;

/// The client side: bitmaps in a Vec, the callbacks counted.
#[derive(Default)]
pub struct VecBitmaps {
    pub store: Vec<Vec<u32>>,
    pub extra: u32,
    pub full: bool,
    pub create: u32,
    pub destroy: u32,
    pub modified: u32,
    pub test_opaque: u32,
    pub set_opaque: u32,
    pub last_opaque: bool,
    width: u32,
}

impl Bitmaps for VecBitmaps {
    type Bitmap = usize;

    fn create(&mut self, width: u32, height: u32) -> Option<usize> {
        self.create += 1;
        self.width = width;
        self.store
            .push(vec![0; ((width + self.extra) * height) as usize]);
        Some(self.store.len() - 1)
    }

    fn destroy(&mut self, _: usize) {
        self.destroy += 1;
    }

    fn pixels(&mut self, bitmap: usize, _: u32, _: u32, _: u32) -> &mut [u32] {
        &mut self.store[bitmap]
    }

    fn rowspan(&mut self, _: usize) -> Option<u32> {
        self.full.then_some(self.width + self.extra)
    }

    fn set_opaque(&mut self, _: usize, opaque: bool) {
        if self.full {
            self.set_opaque += 1;
            self.last_opaque = opaque;
        }
    }

    fn test_opaque(&mut self, _: usize) -> Option<bool> {
        if self.full {
            self.test_opaque += 1;
            Some(true)
        } else {
            None
        }
    }

    fn modified(&mut self, _: usize) {
        if self.full {
            self.modified += 1;
        }
    }
}

/// The GIF writer of the cmocka cases.
#[derive(Default)]
pub struct Writer {
    pub gb: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, v: u32) {
        self.gb.push(v as u8);
    }

    fn u16(&mut self, v: u32) {
        self.u8(v & 0xff);
        self.u8(v >> 8);
    }

    /// Colour i: (i * 16, 255 - i * 16, i * 8).
    fn table(&mut self, bits: u32) {
        for i in 0..(2u32 << bits) {
            self.u8(i * 16);
            self.u8(255 - i * 16);
            self.u8(i * 8);
        }
    }

    pub fn begin(w: u32, h: u32, gct_bits: Option<u32>, bg: u32) -> Self {
        let mut wr = Self::default();
        wr.gb.extend_from_slice(b"GIF89a");
        wr.u16(w);
        wr.u16(h);
        wr.u8(gct_bits.map_or(0, |b| 0x80 | b));
        wr.u8(bg);
        wr.u8(0);
        if let Some(b) = gct_bits {
            wr.table(b);
        }
        wr
    }

    pub fn gce(&mut self, disposal: u32, delay: u32, transparent: Option<u32>) {
        self.gb.extend_from_slice(&[0x21, 0xF9, 4]);
        self.u8((disposal << 2) | u32::from(transparent.is_some()));
        self.u16(delay);
        self.u8(transparent.unwrap_or(0));
        self.u8(0);
    }

    pub fn loops(&mut self, count: u32) {
        self.gb.extend_from_slice(&[0x21, 0xFF, 11]);
        self.gb.extend_from_slice(b"NETSCAPE2.0");
        self.gb.extend_from_slice(&[3, 1]);
        self.u16(count);
        self.u8(0);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn image(
        &mut self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        interlaced: bool,
        lct_bits: Option<u32>,
        index: u32,
    ) {
        self.u8(0x2C);
        self.u16(x);
        self.u16(y);
        self.u16(w);
        self.u16(h);
        self.u8(lct_bits.map_or(0, |b| 0x80 | b) | if interlaced { 0x40 } else { 0 });
        if let Some(b) = lct_bits {
            self.table(b);
        }
        let mut st = Stream::new(8);
        st.clear();
        for _ in 0..w * h {
            st.code(index);
        }
        st.eoi();
        self.u8(8);
        self.gb.extend(st.pack(255));
    }

    pub fn end(mut self) -> Vec<u8> {
        self.u8(0x3B);
        self.gb
    }
}

/// That colour as an R8G8B8A8 pixel on a little-endian host.
pub fn rgba(i: u32) -> u32 {
    (i * 16) | ((255 - i * 16) << 8) | ((i * 8) << 16) | 0xFF00_0000
}

/// The three-frame GIF of most cases (see the cmocka writer).
pub fn three_frames(loops: Option<u32>, disposal1: u32, transparent1: Option<u32>) -> Vec<u8> {
    let mut w = Writer::begin(4, 4, Some(1), 3);
    if let Some(l) = loops {
        w.loops(l);
    }
    w.gce(u32::from(disposal::NONE), 5, None);
    w.image(0, 0, 4, 4, false, None, 1);
    w.gce(disposal1, 7, transparent1);
    w.image(1, 1, 2, 2, false, None, 2);
    w.image(0, 0, 1, 1, false, Some(0), 0);
    w.end()
}

fn scan_all(data: &[u8], bitmaps: VecBitmaps, fmt: BitmapFmt, complete: bool) -> Gif<VecBitmaps> {
    let mut g = Gif::new(bitmaps, fmt as u32);
    assert_eq!(g.data_scan(data), NsgifError::Ok);
    if complete {
        g.data_complete();
    }
    g
}

const R8G8B8A8: BitmapFmt = BitmapFmt::R8G8B8A8;

#[test]
fn gif_strings() {
    let errors = [
        "Success",
        "Out of memory",
        "Invalid source data",
        "Requested frame does not exist",
        "Invalid frame data",
        "Unexpected end of GIF source data",
        "Can't add data to completed GIF",
        "Frame can't be displayed",
        "Animation complete",
    ];
    for (i, e) in errors.iter().enumerate() {
        assert_eq!(gif::strerror(i as u32).to_str(), Ok(*e));
    }
    assert_eq!(gif::strerror(9).to_str(), Ok("Unknown error"));
    assert_eq!(gif::strerror(1000).to_str(), Ok("Unknown error"));
    let disposals = [
        "Unspecified",
        "None",
        "Restore background",
        "Restore previous",
        "Restore quirk",
    ];
    for (i, d) in disposals.iter().enumerate() {
        assert_eq!(gif::str_disposal(i as u32).to_str(), Ok(*d));
    }
    assert_eq!(gif::str_disposal(5).to_str(), Ok("Unspecified"));
    assert_eq!(gif::str_disposal(200).to_str(), Ok("Unspecified"));
}

#[test]
fn gif_create_and_layouts() {
    let cases: [(u32, [u8; 4]); 9] = [
        (BitmapFmt::R8G8B8A8 as u32, [16, 239, 8, 255]),
        (BitmapFmt::B8G8R8A8 as u32, [8, 239, 16, 255]),
        (BitmapFmt::A8R8G8B8 as u32, [255, 16, 239, 8]),
        (BitmapFmt::A8B8G8R8 as u32, [255, 8, 239, 16]),
        (BitmapFmt::Rgba8888 as u32, [255, 8, 239, 16]),
        (BitmapFmt::Bgra8888 as u32, [255, 16, 239, 8]),
        (BitmapFmt::Argb8888 as u32, [8, 239, 16, 255]),
        (BitmapFmt::Abgr8888 as u32, [16, 239, 8, 255]),
        (99, [16, 239, 8, 255]),
    ];
    let data = three_frames(None, u32::from(disposal::NONE), None);
    for (fmt, bytes) in cases {
        let mut g = Gif::new(VecBitmaps::default(), fmt);
        assert_eq!(g.data_scan(&data), NsgifError::Ok);
        let mut table = [0u32; MAX_COLOURS];
        assert_eq!(g.global_palette(&mut table), 4);
        assert_eq!(table[1].to_ne_bytes(), bytes, "format {fmt}");
    }

    let mut w = Writer::begin(2, 1, None, 0);
    w.image(0, 0, 2, 1, false, None, 1);
    let data = w.end();
    let g = scan_all(&data, VecBitmaps::default(), R8G8B8A8, false);
    let mut table = [0u32; MAX_COLOURS];
    assert_eq!(g.global_palette(&mut table), 2);
    assert_eq!(table[0], 0xFF00_0000);
    assert_eq!(table[1], 0xFFFF_FFFF);
    assert!(!g.info().global_palette);
    assert_eq!(g.info().background, 0xFF00_0000);
}

#[test]
fn gif_scan_whole() {
    let data = three_frames(Some(2), u32::from(disposal::RESTORE_BG), None);
    let mut g = scan_all(&data, VecBitmaps::default(), R8G8B8A8, false);
    let info = *g.info();
    assert_eq!(
        (info.width, info.height, info.frame_count, info.loop_max),
        (4, 4, 3, 3)
    );
    assert!(info.global_palette);
    assert_eq!(info.background, rgba(3));

    let frames = [
        (5, disposal::NONE, [0, 0, 4, 4], false),
        (7, disposal::RESTORE_BG, [1, 1, 3, 3], false),
        (10, 0, [0, 0, 1, 1], true),
    ];
    for (f, (delay, d, [x0, y0, x1, y1], local)) in frames.into_iter().enumerate() {
        let fi = *g.frame_info(f as u32).expect("frame");
        assert!(fi.display && !fi.transparency && !fi.interlaced);
        assert_eq!((fi.delay, fi.disposal), (delay, d));
        assert_eq!(
            (fi.rect.x0, fi.rect.y0, fi.rect.x1, fi.rect.y1),
            (x0, y0, x1, y1)
        );
        assert_eq!(fi.local_palette, local);
    }
    assert!(g.frame_info(3).is_none());

    let mut table = [0u32; MAX_COLOURS];
    assert_eq!(g.local_palette(&data, 0, &mut table), None);
    assert_eq!(g.local_palette(&data, 2, &mut table), Some(2));
    assert_eq!(table[1], rgba(1));
    assert_eq!(g.local_palette(&data, 3, &mut table), None);

    assert_eq!(g.data_scan(&data), NsgifError::Ok);
    assert_eq!(g.info().frame_count, 3);
    g.data_complete();
    assert_eq!(g.data_scan(&data), NsgifError::DataComplete);
}

#[test]
fn gif_scan_incremental() {
    let data = three_frames(Some(2), u32::from(disposal::RESTORE_BG), None);
    let whole = scan_all(&data, VecBitmaps::default(), R8G8B8A8, false);

    for k in [13, 25, 44, 50, 52, 61] {
        let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
        assert_eq!(g.data_scan(&data[..k]), NsgifError::EndOfData);
        assert_eq!(g.data_scan(&data), NsgifError::Ok);
        assert_eq!(g.info(), whole.info());
        for f in 0..3 {
            assert_eq!(g.frame_info(f), whole.frame_info(f), "split {k} frame {f}");
        }
    }
    // Kept, characterized, not endorsed: resuming inside a frame.
    for k in [51, 62, 70, 85] {
        let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
        assert_eq!(g.data_scan(&data[..k]), NsgifError::EndOfData);
        assert_eq!(g.data_scan(&data), NsgifError::DataFrame);
        assert_eq!(g.data_scan(&data), NsgifError::DataFrame);
        assert_eq!(g.info().frame_count, 0);
    }
    let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
    for n in 0..=data.len() {
        let expect = if n < 61 {
            NsgifError::EndOfData
        } else {
            NsgifError::DataFrame
        };
        assert_eq!(g.data_scan(&data[..n]), expect, "n {n}");
        assert_eq!(g.info().frame_count, 0);
    }
}

#[test]
fn gif_screen_size() {
    for (w, h) in [(640, 480), (0, 0), (2049, 5)] {
        let mut wr = Writer::begin(w, h, Some(1), 0);
        wr.image(0, 0, 3, 2, false, None, 1);
        let g = scan_all(&wr.end(), VecBitmaps::default(), R8G8B8A8, false);
        assert_eq!((g.info().width, g.info().height), (3, 2));
    }
    let mut wr = Writer::begin(4, 4, Some(1), 0);
    wr.image(0, 0, 6, 5, false, None, 1);
    wr.image(0, 0, 9, 9, false, None, 1);
    let g = scan_all(&wr.end(), VecBitmaps::default(), R8G8B8A8, false);
    assert_eq!((g.info().width, g.info().height), (6, 5));
}

#[test]
fn gif_without_frames() {
    let data = Writer::begin(4, 4, None, 0).end();
    let mut g = scan_all(&data, VecBitmaps::default(), R8G8B8A8, true);
    assert_eq!(g.info().frame_count, 0);
    assert_eq!(g.frame_decode(&data, 0), Err(NsgifError::BadFrame));
    assert_eq!(g.frame_prepare(), Err(NsgifError::FrameDisplay));

    let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
    assert_eq!(g.data_scan(&data[..data.len() - 1]), NsgifError::EndOfData);

    let mut w = Writer::begin(4, 4, None, 0);
    w.gb.extend_from_slice(&[0x2C, 0, 0, 0, 0, 2, 0, 2, 0, 0, 8, 0x3B]);
    let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
    assert_eq!(g.data_scan(&w.gb), NsgifError::Ok);
    assert_eq!(g.info().frame_count, 0);

    let mut w = Writer::begin(4, 4, None, 0);
    w.gb.extend_from_slice(&[0x2C, 0, 0, 0, 0, 2, 0, 2, 0, 0, 12, 1, 0, 0]);
    let data = w.end();
    let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
    assert_eq!(g.data_scan(&data), NsgifError::DataFrame);
    assert_eq!(g.info().frame_count, 0);

    let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
    assert_eq!(g.data_scan(b"PNG89a\0\0\0\0\0\0\0"), NsgifError::Data);
}

#[test]
fn gif_data_complete_partial() {
    let data = three_frames(None, u32::from(disposal::NONE), None);
    let cut = (0..=data.len())
        .find(|&n| {
            let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
            g.data_scan(&data[..n]);
            g.info().frame_count == 2
        })
        .expect("frame 1 complete")
        - 2;

    let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
    assert_eq!(g.data_scan(&data[..cut]), NsgifError::Ok);
    assert_eq!(g.info().frame_count, 1);
    assert!(g.frame_info(1).is_none());
    g.data_complete();
    assert_eq!(g.info().frame_count, 2);
    let f1 = *g.frame_info(1).expect("frame 1");
    assert!(f1.display && !f1.transparency);
    assert!(g.frame_decode(&data[..cut], 1).is_ok());

    let cut0 = 13 + 12 + 19 + 8 + 10 + 3;
    let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
    assert_eq!(g.data_scan(&data[..cut0]), NsgifError::EndOfData);
    assert_eq!(g.info().frame_count, 0);
    g.data_complete();
    assert_eq!(g.info().frame_count, 1);
    let f0 = *g.frame_info(0).expect("frame 0");
    assert!(f0.display && f0.transparency);
    assert!(g.frame_decode(&data[..cut0], 0).is_ok());
}

#[test]
fn gif_prepare_sequence() {
    let data = three_frames(None, u32::from(disposal::NONE), None);
    let mut g = scan_all(&data, VecBitmaps::default(), R8G8B8A8, true);
    assert_eq!(g.info().loop_max, 1);
    let once = [
        (0, 5, [0, 0, 4, 4]),
        (1, 7, [0, 0, 4, 4]),
        (2, INFINITE, [0, 0, 3, 3]),
        (0, 5, [0, 0, 4, 4]),
    ];
    for (frame, delay, [x0, y0, x1, y1]) in once {
        let (area, d, f) = g.frame_prepare().expect("prepare");
        assert_eq!((f, d), (frame, delay));
        assert_eq!((area.x0, area.y0, area.x1, area.y1), (x0, y0, x1, y1));
    }
    assert_eq!(g.frame_prepare(), Err(NsgifError::AnimationEnd));

    assert_eq!(g.reset(), NsgifError::Ok);
    assert_eq!(g.frame_prepare().expect("prepare").2, 0);

    g.set_frame_delay_behaviour(6, 50);
    assert_eq!(g.reset(), NsgifError::Ok);
    assert_eq!(g.frame_prepare().expect("prepare").1, 50);
    assert_eq!(g.frame_prepare().expect("prepare").1, 7);

    let data = three_frames(Some(0), u32::from(disposal::NONE), None);
    let mut g = scan_all(&data, VecBitmaps::default(), R8G8B8A8, true);
    assert_eq!(g.info().loop_max, 0);
    for k in 0..10u32 {
        let (_, d, f) = g.frame_prepare().expect("prepare");
        assert_eq!(f, k % 3);
        assert_ne!(d, INFINITE);
    }

    let mut g = scan_all(&data, VecBitmaps::default(), R8G8B8A8, false);
    for _ in 0..3 {
        assert!(g.frame_prepare().is_ok());
    }
    assert_eq!(g.frame_prepare(), Err(NsgifError::EndOfData));

    let mut w = Writer::begin(2, 2, Some(1), 0);
    w.image(0, 0, 2, 2, false, None, 1);
    let mut g = scan_all(&w.end(), VecBitmaps::default(), R8G8B8A8, true);
    assert_eq!(g.frame_prepare().expect("prepare").1, INFINITE);
}

#[test]
fn gif_decode_disposal() {
    let data = three_frames(None, u32::from(disposal::RESTORE_BG), None);
    let mut g = scan_all(&data, VecBitmaps::default(), R8G8B8A8, true);
    let b = g.frame_decode(&data, 2).expect("decode").expect("bitmap");
    let px = g.bitmaps().store[b].clone();
    for y in 0..4 {
        for x in 0..4 {
            let mut expect = rgba(1);
            if (1..3).contains(&x) && (1..3).contains(&y) {
                expect = rgba(3);
            }
            if x == 0 && y == 0 {
                expect = rgba(0);
            }
            assert_eq!(px[y * 4 + x], expect, "({x}, {y})");
        }
    }
    let b1 = g.frame_decode(&data, 1).expect("decode").expect("bitmap");
    assert_eq!(g.bitmaps().store[b1][0], rgba(1));
    assert_eq!(g.bitmaps().store[b1][5], rgba(2));
    assert_eq!(g.frame_decode(&data, 1), Ok(Some(b1)));
    assert_eq!(g.frame_decode(&data, 3), Err(NsgifError::BadFrame));

    let data = three_frames(None, u32::from(disposal::RESTORE_BG), Some(9));
    let mut g = scan_all(&data, VecBitmaps::default(), R8G8B8A8, true);
    let b = g.frame_decode(&data, 2).expect("decode").expect("bitmap");
    let px = &g.bitmaps().store[b];
    assert_eq!((px[5], px[10], px[15]), (0, 0, rgba(1)));

    let data = three_frames(None, u32::from(disposal::RESTORE_PREV), None);
    let mut g = scan_all(&data, VecBitmaps::default(), R8G8B8A8, true);
    let b = g.frame_decode(&data, 1).expect("decode").expect("bitmap");
    assert_eq!(g.bitmaps().store[b][5], rgba(2));
    let b = g.frame_decode(&data, 2).expect("decode").expect("bitmap");
    let px = &g.bitmaps().store[b];
    assert_eq!((px[0], px[5], px[10]), (rgba(0), rgba(1), rgba(1)));

    let data = three_frames(None, u32::from(disposal::NONE), Some(2));
    let mut g = scan_all(&data, VecBitmaps::default(), R8G8B8A8, true);
    let b = g.frame_decode(&data, 1).expect("decode").expect("bitmap");
    assert_eq!(g.bitmaps().store[b][5], rgba(1));
}

#[test]
fn gif_callbacks() {
    let data = three_frames(None, u32::from(disposal::NONE), None);
    let bitmaps = VecBitmaps {
        extra: 3,
        full: true,
        ..VecBitmaps::default()
    };
    let mut g = scan_all(&data, bitmaps, R8G8B8A8, true);

    let b = g.frame_decode(&data, 0).expect("decode").expect("bitmap");
    let c = g.bitmaps();
    assert_eq!(
        (c.create, c.modified, c.test_opaque, c.set_opaque),
        (1, 1, 1, 1)
    );
    assert!(c.last_opaque);
    let px = &c.store[b];
    assert_eq!(
        (px[0], px[3], px[7], px[3 * 7 + 3]),
        (rgba(1), rgba(1), rgba(1), rgba(1))
    );

    let b = g.frame_decode(&data, 1).expect("decode").expect("bitmap");
    let c = g.bitmaps();
    assert_eq!((c.modified, c.test_opaque, c.set_opaque), (2, 2, 2));
    assert_eq!(c.store[b][7 + 1], rgba(2));

    g.frame_decode(&data, 0).expect("decode").expect("bitmap");
    let c = g.bitmaps();
    assert_eq!(
        (c.create, c.modified, c.test_opaque, c.set_opaque),
        (1, 3, 2, 3)
    );
    drop(g);
}

// ---------------------------------------------------------------------------
// Edges of the safe API.

#[test]
fn edge_drop_destroys_the_bitmap() {
    struct Counting(std::rc::Rc<std::cell::Cell<u32>>, Vec<u32>);
    impl Bitmaps for Counting {
        type Bitmap = ();
        fn create(&mut self, w: u32, h: u32) -> Option<()> {
            self.1 = vec![0; (w * h) as usize];
            Some(())
        }
        fn destroy(&mut self, _: ()) {
            self.0.set(self.0.get() + 1);
        }
        fn pixels(&mut self, _: (), _: u32, _: u32, _: u32) -> &mut [u32] {
            &mut self.1
        }
    }
    let destroyed = std::rc::Rc::new(std::cell::Cell::new(0));
    let data = three_frames(None, u32::from(disposal::NONE), None);
    let mut g = Gif::new(Counting(destroyed.clone(), Vec::new()), R8G8B8A8 as u32);
    assert_eq!(g.data_scan(&data), NsgifError::Ok);
    g.frame_decode(&data, 0).expect("decode").expect("bitmap");
    drop(g);
    assert_eq!(destroyed.get(), 1);
}

#[test]
fn edge_create_fails_is_out_of_memory() {
    struct Full;
    impl Bitmaps for Full {
        type Bitmap = ();
        fn create(&mut self, _: u32, _: u32) -> Option<()> {
            None
        }
        fn destroy(&mut self, _: ()) {}
        fn pixels(&mut self, _: (), _: u32, _: u32, _: u32) -> &mut [u32] {
            &mut []
        }
    }
    let data = three_frames(None, u32::from(disposal::NONE), None);
    let mut g = Gif::new(Full, R8G8B8A8 as u32);
    assert_eq!(g.data_scan(&data), NsgifError::Ok);
    assert_eq!(g.frame_decode(&data, 0), Err(NsgifError::Oom));
}

// ---------------------------------------------------------------------------
// Properties.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn prop_any_data_and_call_order(
        data in prop_oneof![
            proptest::collection::vec(any::<u8>(), 0..200),
            (any::<u8>(), any::<proptest::sample::Index>()).prop_map(|(b, i)| {
                let mut d = three_frames(Some(1), u32::from(disposal::RESTORE_PREV), Some(2));
                let at = i.index(d.len());
                d[at] = b;
                d
            }),
        ],
        cuts in proptest::collection::vec(any::<proptest::sample::Index>(), 0..4),
        ops in proptest::collection::vec(0u8..5, 0..20),
    ) {
        let mut g = Gif::new(VecBitmaps::default(), R8G8B8A8 as u32);
        let mut lens: Vec<usize> = cuts.iter().map(|c| c.index(data.len() + 1)).collect();
        lens.sort_unstable();
        lens.push(data.len());
        let mut count = 0;
        for n in lens {
            let _ = g.data_scan(&data[..n]);
            prop_assert!(g.info().frame_count >= count);
            count = g.info().frame_count;
        }
        let mut table = [0u32; MAX_COLOURS];
        for (k, op) in ops.into_iter().enumerate() {
            match op {
                0 => { let _ = g.frame_prepare(); }
                1 => { let _ = g.frame_decode(&data, k as u32 % 4); }
                2 => g.data_complete(),
                3 => { let _ = g.local_palette(&data, k as u32 % 4, &mut table); }
                _ => { let _ = g.reset(); }
            }
            prop_assert!(g.info().frame_count >= count);
            count = g.info().frame_count;
        }
    }
}

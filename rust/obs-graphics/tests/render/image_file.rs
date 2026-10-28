//! Tier 1 mirror of `test/cmocka/test_image_file.c`, one test per C case,
//! through the safe core.
//!
//! [`Fake`] stands in for everything the file calls: libnsgif plays the
//! fixtures the C test reads (`test/cmocka/data/image-file-*`) from their
//! known frames, delays and loop counts, FFmpeg returns the decoded pixels
//! the C test expects, the allocator counts live blocks like
//! `bnum_allocs`, and the texture calls are logged in the C test's format,
//! so each expected string is the C test's string. Tier 3
//! (`image_file_parity.rs`) runs the real files through the real libnsgif
//! against the original C.

use core::ffi::CStr;
use std::cell::RefCell;

use obs_graphics::image_file::{
    FileRead, GifInfo, Host, ImageAlphaMode, ImageFileEx, Internal, effective_loops, format_bpp,
    frame_time_ns, is_gif_path, premultiply,
};
use obs_graphics::texture_render::{ColorFormat, ColorSpace};

const GS_RGBA: ColorFormat = ColorFormat(3);
const MS: u64 = 1_000_000;

const FRAME0: &str = "00000000.ff0000ff.ff0000ff.ff0000ff.ff0000ff.ff0000ff.ff0000ff.c86432ff";
const FRAME1: &str = "00ff00ff.00ff00ff.00ff00ff.00ff00ff.00ff00ff.00ff00ff.00ff00ff.c86432ff";
const FRAME2: &str = "0000ffff.0000ffff.0000ffff.0000ffff.0000ffff.0000ffff.0000ffff.c86432ff";

fn bytes(hex: &str) -> Vec<u8> {
    hex.split('.')
        .flat_map(|texel| {
            (0..4).map(move |i| u8::from_str_radix(&texel[i * 2..i * 2 + 2], 16).unwrap())
        })
        .collect()
}

fn hex(data: &[u8]) -> String {
    data.chunks(4)
        .map(|t| t.iter().map(|b| format!("{b:02x}")).collect::<String>())
        .collect::<Vec<_>>()
        .join(".")
}

struct FakeGif {
    info: GifInfo,
    frames: Vec<Vec<u8>>,
    delays: Vec<u32>,
    bitmap: Option<u32>,
}

/// Per frame: the block and offset it was copied to.
type FrameCache = Vec<Option<(u32, usize)>>;

#[derive(Default)]
struct State {
    blocks: Vec<Option<Vec<u8>>>,
    caches: Vec<Option<FrameCache>>,
    gifs: Vec<FakeGif>,
    live: i64,
    /// Decoders from gif_create not yet passed to gif_destroy.
    open_gifs: i64,
    textures: Vec<(u32, u32)>,
    log: Vec<String>,
    messages: Vec<String>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

fn setup() {
    with(|s| *s = State::default());
}

fn allocs() -> i64 {
    with(|s| s.live)
}

fn call_log() -> String {
    with(|s| s.log.join(";"))
}

fn clear_log() {
    with(|s| s.log.clear());
}

fn log_has(text: &str) -> bool {
    with(|s| s.messages.iter().any(|m| m.contains(text)))
}

fn new_block(s: &mut State, data: Vec<u8>) -> u32 {
    s.live += 1;
    s.blocks.push(Some(data));
    (s.blocks.len() - 1) as u32
}

fn anim_gif(loop_max: i32) -> FakeGif {
    FakeGif {
        info: GifInfo {
            width: 4,
            height: 2,
            frame_count: 3,
            loop_max,
        },
        frames: vec![bytes(FRAME0), bytes(FRAME1), bytes(FRAME2)],
        delays: vec![10, 0, 5],
        bitmap: None,
    }
}

/// libnsgif's view of the fixture files.
fn open_gif(path: &str) -> Option<FakeGif> {
    match path {
        "image-file-anim.gif" => Some(anim_gif(0)),
        "image-file-loop2.gif" => Some(anim_gif(3)),
        "image-file-noloop.gif" => Some(anim_gif(1)),
        "image-file-still.gif" => Some(FakeGif {
            info: GifInfo {
                width: 2,
                height: 2,
                frame_count: 1,
                loop_max: 1,
            },
            frames: vec![vec![0; 16]],
            delays: vec![0],
            bitmap: None,
        }),
        "image-file-wide.gif" => Some(FakeGif {
            info: GifInfo {
                width: 4097,
                height: 1,
                frame_count: 2,
                loop_max: 0,
            },
            frames: vec![vec![0; 4097 * 4]; 2],
            delays: vec![10, 10],
            bitmap: None,
        }),
        _ => None,
    }
}

const PNG_STRAIGHT: &str = "ff0000ff.00ff0080.0000ffff.0a141eff.28323c00.46505aff";
const PNG_PREMULTIPLIED: &str = "ff0000ff.00800080.0000ffff.0a141eff.00000000.46505aff";

struct Fake;

impl Host for Fake {
    type Texture = u32;
    type Data = u32;
    type Cache = u32;
    type Gif = u32;
    type Bitmap = u32;
    type Internal = Box<Internal<Fake>>;

    fn internal_new() -> Self::Internal {
        with(|s| s.live += 1);
        Box::default()
    }
    fn internal(handle: &mut Self::Internal) -> &mut Internal<Fake> {
        handle
    }
    fn internal_free(_handle: Self::Internal) {
        with(|s| s.live -= 1);
    }

    fn gif_create() -> Option<u32> {
        // The real object is created by scanning; this one is a slot.
        with(|s| {
            s.open_gifs += 1;
            s.gifs.push(FakeGif {
                info: GifInfo::default(),
                frames: Vec::new(),
                delays: Vec::new(),
                bitmap: None,
            });
            Some((s.gifs.len() - 1) as u32)
        })
    }
    fn read_file(path: &CStr) -> FileRead<u32> {
        let name = path.to_str().unwrap();
        let Some(gif) = open_gif(name) else {
            return FileRead::OpenFailed;
        };
        with(|s| {
            let last = s.gifs.len() - 1;
            s.gifs[last] = gif;
            FileRead::Read {
                data: new_block(s, vec![0; 64]),
                size: 64,
                complete: true,
            }
        })
    }
    fn gif_scan(_gif: Option<u32>, _size: usize, _data: Option<u32>) -> u32 {
        0
    }
    fn gif_complete(_gif: Option<u32>) {}
    fn gif_info(gif: Option<u32>) -> GifInfo {
        with(|s| s.gifs[gif.unwrap() as usize].info)
    }
    fn gif_decode(gif: Option<u32>, frame: u32) -> (u32, Option<u32>) {
        with(|s| {
            let g = gif.unwrap() as usize;
            if frame >= s.gifs[g].info.frame_count {
                return (3, None);
            }
            let pixels = s.gifs[g].frames[frame as usize].clone();
            let bitmap = match s.gifs[g].bitmap {
                Some(b) => {
                    s.blocks[b as usize] = Some(pixels);
                    b
                }
                None => {
                    let b = new_block(s, pixels);
                    s.gifs[g].bitmap = Some(b);
                    b
                }
            };
            (0, Some(bitmap))
        })
    }
    fn gif_frame_delay(gif: Option<u32>, frame: i32) -> u32 {
        with(|s| s.gifs[gif.unwrap() as usize].delays[frame as usize])
    }
    fn gif_destroy(gif: Option<u32>) {
        with(|s| {
            s.open_gifs -= 1;
            if let Some(b) = s.gifs[gif.unwrap() as usize].bitmap.take() {
                s.blocks[b as usize] = None;
                s.live -= 1;
            }
        })
    }
    fn with_bitmap(bitmap: u32, len: usize, f: &mut dyn FnMut(&mut [u8])) {
        let mut data = with(|s| s.blocks[bitmap as usize].take().unwrap());
        f(&mut data[..len]);
        with(|s| s.blocks[bitmap as usize] = Some(data));
    }

    fn alloc_data(size: usize) -> u32 {
        with(|s| new_block(s, vec![0; size]))
    }
    fn alloc_cache(count: u32) -> u32 {
        with(|s| {
            s.live += 1;
            s.caches.push(Some(vec![None; count as usize]));
            (s.caches.len() - 1) as u32
        })
    }
    fn free_data(data: Option<u32>) {
        if let Some(d) = data {
            with(|s| {
                assert!(s.blocks[d as usize].take().is_some(), "double free");
                s.live -= 1;
            })
        }
    }
    fn free_cache(cache: Option<u32>) {
        if let Some(c) = cache {
            with(|s| {
                assert!(s.caches[c as usize].take().is_some(), "double free");
                s.live -= 1;
            })
        }
    }
    fn cache_has(cache: Option<u32>, frame: i32) -> bool {
        with(|s| s.caches[cache.unwrap() as usize].as_ref().unwrap()[frame as usize].is_some())
    }
    fn cache_store(
        cache: Option<u32>,
        frame: i32,
        frame_data: Option<u32>,
        offset: usize,
        bitmap: Option<u32>,
        len: usize,
    ) {
        with(|s| {
            let src = s.blocks[bitmap.unwrap() as usize].as_ref().unwrap()[..len].to_vec();
            let dst = frame_data.unwrap();
            s.blocks[dst as usize].as_mut().unwrap()[offset..offset + len].copy_from_slice(&src);
            s.caches[cache.unwrap() as usize].as_mut().unwrap()[frame as usize] =
                Some((dst, offset));
        })
    }

    fn decode_file(
        path: &CStr,
        alpha_mode: ImageAlphaMode,
        format: &mut ColorFormat,
        cx: &mut u32,
        cy: &mut u32,
        space: &mut ColorSpace,
    ) -> Option<u32> {
        let (w, h, pixels) = match (path.to_str().unwrap(), alpha_mode) {
            ("image-file-still.gif", _) => (2, 2, vec![0; 16]),
            ("image-file.png", ImageAlphaMode::PREMULTIPLY) => (3, 2, bytes(PNG_PREMULTIPLIED)),
            ("image-file.png", _) => (3, 2, bytes(PNG_STRAIGHT)),
            ("solid-1024x1024.png", _) => (1024, 1024, vec![0; 1024 * 1024 * 4]),
            _ => return None,
        };
        *format = GS_RGBA;
        *cx = w;
        *cy = h;
        *space = ColorSpace(0);
        Some(with(|s| new_block(s, pixels)))
    }
    fn texture_create_bitmap(
        cx: u32,
        cy: u32,
        format: ColorFormat,
        bitmap: Option<u32>,
    ) -> Option<u32> {
        Some(create_texture(cx, cy, format, 0x2, bitmap))
    }
    fn texture_create_data(
        cx: u32,
        cy: u32,
        format: ColorFormat,
        data: Option<u32>,
    ) -> Option<u32> {
        Some(create_texture(cx, cy, format, 0, data))
    }
    fn texture_destroy(texture: Option<u32>) {
        // graphics.c returns before the device for null.
        if let Some(t) = texture {
            with(|s| s.log.push(format!("tex_destroy T{t}")));
        }
    }
    fn texture_set_image(texture: Option<u32>, cache: Option<u32>, frame: i32, linesize: u32) {
        with(|s| {
            let Some(t) = texture else { return };
            let Some((block, offset)) =
                s.caches[cache.unwrap() as usize].as_ref().unwrap()[frame as usize]
            else {
                return;
            };
            let (cx, cy) = s.textures[t as usize - 1];
            assert_eq!(linesize, cx * 4);
            let len = (cx * cy * 4) as usize;
            let data = &s.blocks[block as usize].as_ref().unwrap()[offset..offset + len];
            let line = format!("set_image T{t} {}", hex(data));
            s.log.push(line);
        })
    }

    fn log(_level: i32, message: &[u8]) {
        with(|s| {
            s.messages
                .push(String::from_utf8_lossy(message).into_owned())
        });
    }
}

fn create_texture(cx: u32, cy: u32, format: ColorFormat, flags: u32, data: Option<u32>) -> u32 {
    with(|s| {
        s.textures.push((cx, cy));
        let t = s.textures.len() as u32;
        let pixels = match data {
            Some(d) => hex(&s.blocks[d as usize].as_ref().unwrap()[..(cx * cy * 4) as usize]),
            None => "null".into(),
        };
        s.log.push(format!(
            "tex_create T{t} {cx} {cy} f{} l1 0x{flags:x} {pixels}",
            format.0
        ));
        t
    })
}

type Image = ImageFileEx<Fake>;

fn image() -> Image {
    Image::default()
}

fn path(name: &str) -> std::ffi::CString {
    std::ffi::CString::new(name).unwrap()
}

fn is_zeroed(image: &Image) -> bool {
    image.texture.is_none()
        && image.format == ColorFormat(0)
        && image.cx == 0
        && image.cy == 0
        && !image.is_animated_gif
        && !image.frame_updated
        && !image.loaded
        && image.internal.is_none()
        && image.cur_time == 0
        && image.cur_frame == 0
        && image.cur_loop == 0
        && image.texture_data.is_none()
        && image.mem_usage == 0
        && image.alpha_mode == ImageAlphaMode::STRAIGHT
        && image.space == ColorSpace(0)
}

#[test]
fn null_file_test() {
    setup();
    let mut image = image();
    image.cx = 0x5a5a_5a5a;
    image.loaded = true;
    image.init(None, ImageAlphaMode::STRAIGHT);
    assert!(is_zeroed(&image));
    assert_eq!(allocs(), 0);

    image.init_texture();
    assert!(!image.tick(100 * MS));
    image.update_texture();
    image.free();
    assert!(is_zeroed(&image));
    assert_eq!(call_log(), "");
    assert_eq!(allocs(), 0);
}

#[test]
fn missing_gif_test() {
    setup();
    let mut image = image();
    image.init(
        Some(&path("image-file-missing.gif")),
        ImageAlphaMode::STRAIGHT,
    );
    assert!(is_zeroed(&image));
    assert_eq!(allocs(), 0);
    assert!(log_has(
        "init_animated_gif: Failed to open file 'image-file-missing.gif'"
    ));
    assert!(!log_has("Failed to load file"));
    assert_eq!(with(|s| s.open_gifs), 0);
    image.free();
    assert_eq!(call_log(), "");
}

#[test]
fn missing_png_test() {
    setup();
    let mut image = image();
    image.init(
        Some(&path("image-file-missing.png")),
        ImageAlphaMode::STRAIGHT,
    );
    assert!(is_zeroed(&image));
    assert_eq!(allocs(), 0);
    assert!(log_has(
        "gs_image_file_ex_init_internal: Failed to load file 'image-file-missing.png'"
    ));
    assert!(!log_has("init_animated_gif"));
}

#[test]
fn wide_gif_test() {
    setup();
    let mut image = image();
    image.init(Some(&path("image-file-wide.gif")), ImageAlphaMode::STRAIGHT);
    assert!(is_zeroed(&image));
    assert_eq!(allocs(), 0);
    assert!(log_has(
        "init_animated_gif: Bad texture dimensions (4097x1) in 'image-file-wide.gif'"
    ));
    assert!(!log_has("Failed to load file"));
    assert_eq!(with(|s| s.open_gifs), 0);
}

#[test]
fn animated_gif_load_test() {
    setup();
    let mut image = image();
    image.init(
        Some(&path("image-file-anim.gif")),
        ImageAlphaMode::PREMULTIPLY,
    );
    assert!(image.loaded);
    assert!(image.is_animated_gif);
    assert!(!image.frame_updated);
    assert_eq!(image.cx, 4);
    assert_eq!(image.cy, 2);
    assert_eq!(image.format, GS_RGBA);
    assert!(image.internal.is_some());
    assert!(image.texture.is_none());
    assert!(image.texture_data.is_none());
    assert_eq!(image.cur_time, 0);
    assert_eq!(image.cur_frame, 0);
    assert_eq!(image.cur_loop, 0);
    // frame cache (3 pointers), 3 decoded frames, the current frame, the
    // file (the fake reads 64 bytes)
    assert_eq!(
        image.mem_usage,
        (3 * size_of::<usize>() + 3 * (4 * 2 * 4) + 4 * 2 * 4 + 64) as u64
    );
    assert_eq!(image.alpha_mode, ImageAlphaMode::STRAIGHT);
    assert_eq!(image.space, ColorSpace(0));
    // internal, file data, nsgif's bitmap, frame cache, frame data
    assert_eq!(allocs(), 5);
    assert_eq!(call_log(), "");

    image.init_texture();
    assert_eq!(image.texture, Some(1));
    assert_eq!(call_log(), format!("tex_create T1 4 2 f3 l1 0x2 {FRAME0}"));
    assert_eq!(allocs(), 5);

    clear_log();
    image.free();
    assert!(is_zeroed(&image));
    assert_eq!(call_log(), "tex_destroy T1");
    assert_eq!(allocs(), 0);
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn tick_test() {
    setup();
    let mut image = image();
    image.init(Some(&path("image-file-anim.gif")), ImageAlphaMode::STRAIGHT);
    image.init_texture();
    clear_log();

    // frame 0: 100 ms
    assert!(!image.tick(50 * MS));
    assert_eq!(image.cur_time, 50 * MS);
    assert!(!image.tick(50 * MS));
    assert_eq!(image.cur_frame, 0);
    assert_eq!(image.cur_time, 100 * MS);
    assert!(image.tick(1));
    assert_eq!(image.cur_frame, 1);
    assert_eq!(image.cur_time, 1);

    image.update_texture();
    assert_eq!(call_log(), format!("set_image T1 {FRAME1}"));
    clear_log();

    // frame 1: delay 0, so 100 ms
    assert!(!image.tick(100 * MS - 1));
    assert_eq!(image.cur_frame, 1);
    assert!(image.tick(50 * MS));
    assert_eq!(image.cur_frame, 2);
    assert_eq!(image.cur_time, 50 * MS);

    // frame 2: 50 ms, then frame 0 again; the loop count stays 0.
    assert!(image.tick(1));
    assert_eq!(image.cur_frame, 0);
    assert_eq!(image.cur_loop, 0);
    assert_eq!(image.cur_time, 1);

    // Exactly one full loop (250 ms) comes back to frame 0: false.
    assert!(!image.tick(250 * MS));
    assert_eq!(image.cur_frame, 0);
    assert_eq!(image.cur_time, 1);

    // Frame 2 comes from the cache.
    assert!(image.tick(200 * MS));
    assert_eq!(image.cur_frame, 2);
    assert_eq!(image.cur_time, 1);
    image.update_texture();
    assert_eq!(call_log(), format!("set_image T1 {FRAME2}"));
    clear_log();

    assert!(image.tick(50 * MS));
    assert_eq!(image.cur_frame, 0);
    image.update_texture();
    assert_eq!(call_log(), format!("set_image T1 {FRAME0}"));

    image.free();
}

#[test]
fn skip_frames_test() {
    setup();
    let mut image = image();
    // Frame 1 is decoded on the way to frame 2, then frame 2 is shown.
    image.init(Some(&path("image-file-anim.gif")), ImageAlphaMode::STRAIGHT);
    image.init_texture();
    clear_log();
    assert!(image.tick(200 * MS + 1));
    assert_eq!(image.cur_frame, 2);
    assert_eq!(image.cur_time, 1);
    image.update_texture();
    assert_eq!(call_log(), format!("set_image T1 {FRAME2}"));
    image.free();
}

#[test]
fn loop_count_test() {
    setup();
    let mut image = image();

    // NETSCAPE loop count 2: libnsgif reports loop_max 3, three plays.
    image.init(
        Some(&path("image-file-loop2.gif")),
        ImageAlphaMode::STRAIGHT,
    );
    assert!(image.is_animated_gif);

    assert!(!image.tick(250 * MS + 1));
    assert_eq!(image.cur_frame, 0);
    assert_eq!(image.cur_loop, 1);
    assert!(!image.tick(250 * MS));
    assert_eq!(image.cur_frame, 0);
    assert_eq!(image.cur_loop, 2);
    assert_eq!(image.cur_time, 1);

    // The third play ends on its last frame.
    assert!(image.tick(1000 * MS));
    assert_eq!(image.cur_frame, 2);
    assert_eq!(image.cur_loop, 3);
    assert_eq!(image.cur_time, 750 * MS + 1);

    assert!(!image.tick(1000 * MS));
    assert_eq!(image.cur_frame, 2);
    assert_eq!(image.cur_time, 750 * MS + 1);
    image.free();

    // No NETSCAPE block: one play.
    image.init(
        Some(&path("image-file-noloop.gif")),
        ImageAlphaMode::STRAIGHT,
    );
    assert!(image.tick(100 * MS + 1));
    assert_eq!(image.cur_frame, 1);
    assert!(image.tick(150 * MS));
    assert_eq!(image.cur_frame, 2);
    assert_eq!(image.cur_loop, 1);
    assert_eq!(image.cur_time, 1);
    assert!(!image.tick(1000 * MS));
    assert_eq!(image.cur_frame, 2);
    assert_eq!(image.cur_time, 1);
    image.free();
}

#[test]
fn still_gif_test() {
    setup();
    let mut image = image();
    // One frame: not animated, so FFmpeg decodes it.
    image.init(
        Some(&path("image-file-still.gif")),
        ImageAlphaMode::STRAIGHT,
    );
    assert!(image.loaded);
    assert!(!image.is_animated_gif);
    assert_eq!(image.cx, 2);
    assert_eq!(image.cy, 2);
    assert_eq!(image.mem_usage, 2 * 2 * 4);
    assert!(image.texture_data.is_some());
    assert!(image.internal.is_some());
    // internal, texture data
    assert_eq!(allocs(), 2);
    assert!(!image.tick(1000 * MS));
    image.update_texture();
    assert_eq!(call_log(), "");
    image.free();
    assert!(is_zeroed(&image));
    assert_eq!(allocs(), 0);
}

#[test]
fn png_test() {
    setup();
    let mut image = image();
    image.init(Some(&path("image-file.png")), ImageAlphaMode::STRAIGHT);
    assert!(image.loaded);
    assert!(!image.is_animated_gif);
    assert_eq!(image.cx, 3);
    assert_eq!(image.cy, 2);
    assert_eq!(image.format, GS_RGBA);
    assert_eq!(image.space, ColorSpace(0));
    assert_eq!(image.mem_usage, 3 * 2 * 4);
    assert!(image.texture_data.is_some());
    assert_eq!(allocs(), 2);

    image.init_texture();
    assert!(image.texture_data.is_none());
    assert_eq!(allocs(), 1);
    assert_eq!(
        call_log(),
        format!("tex_create T1 3 2 f3 l1 0x0 {PNG_STRAIGHT}")
    );
    clear_log();
    image.free();
    assert_eq!(call_log(), "tex_destroy T1");
    assert_eq!(allocs(), 0);

    image.init(Some(&path("image-file.png")), ImageAlphaMode::PREMULTIPLY);
    clear_log();
    image.init_texture();
    assert_eq!(
        call_log(),
        format!("tex_create T2 3 2 f3 l1 0x0 {PNG_PREMULTIPLIED}")
    );
    image.free();
}

/// obs-rust/obs-studio#2279: a 1024x1024 picture counts its 4 MiB. The fake
/// decoder answers for this name, so no file is read.
#[test]
fn png_1024_mem_usage_test() {
    setup();
    let mut image = image();
    image.init(
        Some(&path("solid-1024x1024.png")),
        ImageAlphaMode::PREMULTIPLY,
    );
    assert!(image.loaded);
    assert_eq!((image.cx, image.cy), (1024, 1024));
    assert!(image.mem_usage >= 4 << 20);
    assert_eq!(image.mem_usage, 1024 * 1024 * 4);
    image.free();
    assert!(is_zeroed(&image));
    assert_eq!(allocs(), 0);
}

/// A file that fails to load counts nothing.
#[test]
fn failed_load_counts_nothing() {
    setup();
    let mut image = image();
    image.mem_usage = 7;
    image.init(
        Some(&path("image-file-missing.png")),
        ImageAlphaMode::STRAIGHT,
    );
    assert_eq!(image.mem_usage, 0);
    image.init(Some(&path("image-file-wide.gif")), ImageAlphaMode::STRAIGHT);
    assert_eq!(image.mem_usage, 0);
}

// New edge cases of the helpers.

#[test]
fn gif_path_suffix() {
    assert!(is_gif_path(b"a.gif"));
    assert!(is_gif_path(b"a.GiF"));
    assert!(!is_gif_path(b".gif"));
    assert!(!is_gif_path(b"a.png"));
    assert!(!is_gif_path(b"gif"));
}

#[test]
fn frame_time_and_loops() {
    assert_eq!(frame_time_ns(0), 100 * MS);
    assert_eq!(frame_time_ns(1), 10 * MS);
    assert_eq!(frame_time_ns(u32::MAX), u64::from(u32::MAX) * 10 * MS);
    assert_eq!(effective_loops(0), 0);
    assert_eq!(effective_loops(3), 3);
    assert_eq!(effective_loops(0xFFFE), 0xFFFE);
    assert_eq!(effective_loops(0xFFFF), 0);
    assert_eq!(effective_loops(-1), -1);
}

#[test]
fn format_bits_per_pixel() {
    let expected = [
        0, 8, 8, 32, 32, 32, 32, 64, 16, 64, 128, 32, 64, 16, 32, 4, 8, 8, 16, 32, 32, 32, 32,
    ];
    for (format, bpp) in expected.iter().enumerate() {
        assert_eq!(
            format_bpp(ColorFormat(format as u32)),
            *bpp,
            "format {format}"
        );
    }
    assert_eq!(format_bpp(ColorFormat(23)), 0);
}

#[test]
fn premultiply_texels() {
    let mut data = bytes("ff8040ff.ff804080.ff804000");
    premultiply(&mut data, false);
    assert_eq!(hex(&data), "ff8040ff.80402080.00000000");

    let mut data = bytes("ff8040ff.ff804000");
    premultiply(&mut data, true);
    assert_eq!(hex(&data), "ff8040ff.00000000");
}

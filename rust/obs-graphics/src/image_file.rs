//! Port of `libobs/graphics/image-file.c.h`: `gs_image_file_ex_*`.
//!
//! An image file loaded for a texture. A `.gif` with more than one frame is
//! decoded by libnsgif and animated by elapsed time; anything else, a
//! one-frame GIF included, is decoded by `gs_create_texture_file_data3`
//! (graphics-ffmpeg). [`ImageFileEx`] is `struct gs_image_file_ex` from the
//! public `graphics/image-file.h`, generic over the handles a [`Host`]
//! hands out: the C ABI shim uses the real pointers, tests use fakes.
//!
//! Everything outside the file stays where it is and is reached through
//! [`Host`]: libnsgif, the file read, `bmalloc`/`bzalloc`/`bfree`, the
//! graphics calls and the log. The host calls are made in the C order, with
//! the C arguments, so a recording host sees the same sequence.
//!
//! Characterized, not endorsed (kept as the C does it):
//! - A `.gif` that cannot be opened or read, cannot be scanned, or is larger
//!   than 4096 in either direction fails without trying FFmpeg. Its nsgif
//!   object is not destroyed.
//! - Init never fills `alpha_mode` or `space`.
//! - Only frame 0 gets the alpha mode given to init; frames decoded later by
//!   tick and update are straight alpha.
//! - A GIF that loops forever never counts `cur_loop` up.
//! - A tick that comes back to the current frame returns false and decodes
//!   nothing.
//! - A frame delay of 0 plays for 100 ms.
//! - Scanning repeats until libnsgif reports success, so a scan error that
//!   persists does not end the loop.

use core::ffi::CStr;

use crate::texture_render::{ColorFormat, ColorSpace};

/// `enum gs_image_alpha_mode`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImageAlphaMode(pub u32);

impl ImageAlphaMode {
    pub const STRAIGHT: Self = Self(0);
    pub const PREMULTIPLY_SRGB: Self = Self(1);
    pub const PREMULTIPLY: Self = Self(2);
}

/// `GS_RGBA`.
pub const GS_RGBA: ColorFormat = ColorFormat(3);
/// `GS_DYNAMIC`, the flag of an animated GIF's texture.
pub const GS_DYNAMIC: u32 = 1 << 1;
/// `NSGIF_OK`.
pub const NSGIF_OK: u32 = 0;
/// `LOG_WARNING`.
pub const LOG_WARNING: i32 = 200;

/// The `nsgif_info_t` fields the file reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GifInfo {
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    pub loop_max: i32,
}

/// What reading the whole file gave, in the C steps: open, size, allocate,
/// read.
pub enum FileRead<D> {
    /// `os_fopen` failed; nothing was allocated.
    OpenFailed,
    /// The file's size and the `bmalloc` block of that size; `complete` is
    /// whether `fread` filled it.
    Read {
        data: D,
        size: usize,
        complete: bool,
    },
}

/// The calls the file makes outside itself. Handles are plain values the
/// host owns; the core never looks inside them.
pub trait Host: Sized {
    /// `gs_texture_t *`.
    type Texture: Copy;
    /// A `bmalloc`/`bzalloc` byte block.
    type Data: Copy;
    /// The `uint8_t **` frame cache: one pointer per frame into the frame
    /// data, null until that frame is decoded.
    type Cache: Copy;
    /// `nsgif_t *`.
    type Gif: Copy;
    /// `nsgif_bitmap_t *`: libnsgif's current frame, `4 * width * height`
    /// RGBA bytes.
    type Bitmap: Copy;
    /// `struct gs_image_file_internal *`, a `bzalloc` block.
    type Internal;

    /// `bzalloc(sizeof(struct gs_image_file_internal))`.
    fn internal_new() -> Self::Internal;
    /// The block behind an internal handle.
    fn internal(handle: &mut Self::Internal) -> &mut Internal<Self>;
    /// `bfree` of the internal block.
    fn internal_free(handle: Self::Internal);

    /// `nsgif_create` with the file's bitmap callbacks, RGBA.
    fn gif_create() -> Option<Self::Gif>;
    /// `os_fopen` .. `fread` of the whole file.
    fn read_file(path: &CStr) -> FileRead<Self::Data>;
    /// `nsgif_data_scan`.
    fn gif_scan(gif: Option<Self::Gif>, size: usize, data: Option<Self::Data>) -> u32;
    /// `nsgif_data_complete`.
    fn gif_complete(gif: Option<Self::Gif>);
    /// `*nsgif_get_info`.
    fn gif_info(gif: Option<Self::Gif>) -> GifInfo;
    /// `nsgif_frame_decode`: the error, and the bitmap when it is `NSGIF_OK`.
    fn gif_decode(gif: Option<Self::Gif>, frame: u32) -> (u32, Option<Self::Bitmap>);
    /// `nsgif_get_frame_info(gif, frame)->delay`, in centiseconds.
    fn gif_frame_delay(gif: Option<Self::Gif>, frame: i32) -> u32;
    /// `nsgif_destroy`.
    fn gif_destroy(gif: Option<Self::Gif>);
    /// Runs `f` over the `len` bytes of a bitmap.
    fn with_bitmap(bitmap: Self::Bitmap, len: usize, f: &mut dyn FnMut(&mut [u8]));

    /// `bzalloc(size)`.
    fn alloc_data(size: usize) -> Self::Data;
    /// `bzalloc(count * sizeof(uint8_t *))`.
    fn alloc_cache(count: u32) -> Self::Cache;
    /// `bfree`.
    fn free_data(data: Option<Self::Data>);
    /// `bfree`.
    fn free_cache(cache: Option<Self::Cache>);
    /// Whether `cache[frame]` is set.
    fn cache_has(cache: Option<Self::Cache>, frame: i32) -> bool;
    /// `cache[frame] = frame_data + offset`, then the `len` bitmap bytes
    /// copied there.
    fn cache_store(
        cache: Option<Self::Cache>,
        frame: i32,
        frame_data: Option<Self::Data>,
        offset: usize,
        bitmap: Option<Self::Bitmap>,
        len: usize,
    );

    /// `gs_create_texture_file_data3`, which writes the outputs only on
    /// success.
    fn decode_file(
        path: &CStr,
        alpha_mode: ImageAlphaMode,
        format: &mut ColorFormat,
        cx: &mut u32,
        cy: &mut u32,
        space: &mut ColorSpace,
    ) -> Option<Self::Data>;
    /// `gs_texture_create(cx, cy, format, 1, &bitmap, GS_DYNAMIC)`.
    fn texture_create_bitmap(
        cx: u32,
        cy: u32,
        format: ColorFormat,
        bitmap: Option<Self::Bitmap>,
    ) -> Option<Self::Texture>;
    /// `gs_texture_create(cx, cy, format, 1, &data, 0)`.
    fn texture_create_data(
        cx: u32,
        cy: u32,
        format: ColorFormat,
        data: Option<Self::Data>,
    ) -> Option<Self::Texture>;
    /// `gs_texture_destroy`, also with null.
    fn texture_destroy(texture: Option<Self::Texture>);
    /// `gs_texture_set_image(texture, cache[frame], linesize, false)`.
    fn texture_set_image(
        texture: Option<Self::Texture>,
        cache: Option<Self::Cache>,
        frame: i32,
        linesize: u32,
    );

    /// `blog(level, ...)` of the formatted line, raw bytes as `%s` prints.
    fn log(level: i32, message: &[u8]);
}

/// `struct gs_image_file_internal`, private to the file.
pub struct Internal<H: Host> {
    pub gif: Option<H::Gif>,
    pub gif_info: GifInfo,
    pub gif_frame_image: Option<H::Bitmap>,
    pub gif_data: Option<H::Data>,
    pub animation_frame_cache: Option<H::Cache>,
    pub animation_frame_data: Option<H::Data>,
    pub last_decoded_frame: i32,
}

impl<H: Host> Default for Internal<H> {
    fn default() -> Self {
        Self {
            gif: None,
            gif_info: GifInfo::default(),
            gif_frame_image: None,
            gif_data: None,
            animation_frame_cache: None,
            animation_frame_data: None,
            last_decoded_frame: 0,
        }
    }
}

/// `struct gs_image_file_ex`, field for field.
#[repr(C)]
pub struct ImageFileEx<H: Host> {
    pub texture: Option<H::Texture>,
    pub format: ColorFormat,
    pub cx: u32,
    pub cy: u32,
    pub is_animated_gif: bool,
    pub frame_updated: bool,
    pub loaded: bool,
    pub internal: Option<H::Internal>,
    pub cur_time: u64,
    pub cur_frame: i32,
    pub cur_loop: i32,
    pub texture_data: Option<H::Data>,
    pub mem_usage: u64,
    pub alpha_mode: ImageAlphaMode,
    pub space: ColorSpace,
}

impl<H: Host> Default for ImageFileEx<H> {
    fn default() -> Self {
        Self {
            texture: None,
            format: ColorFormat(0),
            cx: 0,
            cy: 0,
            is_animated_gif: false,
            frame_updated: false,
            loaded: false,
            internal: None,
            cur_time: 0,
            cur_frame: 0,
            cur_loop: 0,
            texture_data: None,
            mem_usage: 0,
            alpha_mode: ImageAlphaMode::STRAIGHT,
            space: ColorSpace(0),
        }
    }
}

/// `gs_get_format_bpp`.
pub fn format_bpp(format: ColorFormat) -> u32 {
    match format.0 {
        15 => 4,                                           // DXT1
        1 | 2 | 16 | 17 => 8,                              // A8 R8 DXT3 DXT5
        8 | 13 | 18 => 16,                                 // R16 R16F R8G8
        3 | 4 | 5 | 6 | 11 | 14 | 19 | 20 | 21 | 22 => 32, // RGBA .. RG16
        7 | 9 | 12 => 64,                                  // RGBA16 RGBA16F RG32F
        10 => 128,                                         // RGBA32F
        _ => 0,
    }
}

fn srgb_nonlinear_to_linear(u: f32) -> f32 {
    if u <= 0.04045 {
        u / 12.92
    } else {
        ((u + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_linear_to_nonlinear(u: f32) -> f32 {
    if u <= 0.003_130_8 {
        12.92 * u
    } else {
        (1.055 * u.powf(1.0 / 2.4)) - 0.055
    }
}

fn float_to_u8(f: f32) -> u8 {
    // The C `(uint8_t)(f * 255.0f + 0.5f)`; the value is in 0..=255.5.
    (f * 255.0 + 0.5) as u8
}

/// `gs_premultiply_xyza_loop` (`srgb` false) and
/// `gs_premultiply_xyza_srgb_loop` (`srgb` true) over whole texels.
pub fn premultiply(data: &mut [u8], srgb: bool) {
    for texel in data.as_chunks_mut::<4>().0 {
        let mut f = [0f32; 4];
        for (f, u) in f.iter_mut().zip(texel.iter()) {
            *f = f32::from(*u) / 255.0;
        }
        if srgb {
            for c in &mut f[..3] {
                *c = srgb_nonlinear_to_linear(*c);
            }
        }
        let a = f[3];
        for c in &mut f[..3] {
            *c *= a;
        }
        if srgb {
            for c in &mut f[..3] {
                *c = srgb_linear_to_nonlinear(*c);
            }
        }
        for (u, f) in texel.iter_mut().zip(&f[..3]) {
            *u = float_to_u8(*f);
        }
    }
}

fn premultiply_for<H: Host>(bitmap: Option<H::Bitmap>, texels: usize, alpha_mode: ImageAlphaMode) {
    let srgb = match alpha_mode {
        ImageAlphaMode::PREMULTIPLY_SRGB => true,
        ImageAlphaMode::PREMULTIPLY => false,
        _ => return,
    };
    if let Some(bitmap) = bitmap {
        H::with_bitmap(bitmap, texels * 4, &mut |bytes| premultiply(bytes, srgb));
    }
}

/// Whether the path ends in `.gif`, any case, with something before it.
pub fn is_gif_path(path: &[u8]) -> bool {
    path.len() > 4
        && path
            .get(path.len() - 4..)
            .is_some_and(|ext| ext.eq_ignore_ascii_case(b".gif"))
}

/// `get_time`: a frame delay in centiseconds as nanoseconds, 0 as 100 ms.
pub fn frame_time_ns(delay: u32) -> u64 {
    match u64::from(delay) * 10_000_000 {
        0 => 100_000_000,
        val => val,
    }
}

/// The C `int get_full_decoded_gif_size`: the `uint32_t` product, converted
/// to `int`.
fn full_decoded_gif_size(info: &GifInfo) -> i32 {
    info.width
        .wrapping_mul(info.height)
        .wrapping_mul(4)
        .wrapping_mul(info.frame_count) as i32
}

/// One log line, built on the stack and cut at `blog`'s 4096 bytes. The
/// file allocates nothing on the Rust heap and has no panic path: an
/// executable that links the Rust staticlib next to libobs (as
/// test_effect_parser_lifecycle_rust does) would otherwise pull in Rust's
/// allocator and std, which clash with libobs on Windows.
struct Line {
    buf: [u8; 4096],
    len: usize,
}

impl Line {
    fn new() -> Self {
        Self {
            buf: [0; 4096],
            len: 0,
        }
    }
    fn push(&mut self, bytes: &[u8]) -> &mut Self {
        for &b in bytes {
            if let Some(slot) = self.buf.get_mut(self.len) {
                *slot = b;
                self.len += 1;
            }
        }
        self
    }
    fn push_u32(&mut self, value: u32) -> &mut Self {
        let mut digits = [0u8; 10];
        let mut n = value;
        let mut start = digits.len();
        loop {
            start -= 1;
            if let Some(d) = digits.get_mut(start) {
                *d = b'0' + (n % 10) as u8;
            }
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.push(digits.get(start..).unwrap_or_default())
    }
    fn push_i32(&mut self, value: i32) -> &mut Self {
        if value < 0 {
            self.push(b"-");
        }
        self.push_u32(value.unsigned_abs())
    }
    fn send<H: Host>(&self) {
        H::log(LOG_WARNING, self.buf.get(..self.len).unwrap_or_default());
    }
}

fn log_path<H: Host>(function: &str, before: &str, path: &CStr, after: &str) {
    Line::new()
        .push(function.as_bytes())
        .push(b": ")
        .push(before.as_bytes())
        .push(path.to_bytes())
        .push(after.as_bytes())
        .send::<H>();
}

/// The animation timing of `calculate_new_frame`, over the public fields.
/// `delay` is `get_time` of a frame. Returns the new frame.
pub fn calculate_new_frame(
    cur_time: &mut u64,
    cur_loop: &mut i32,
    cur_frame: i32,
    elapsed_time_ns: u64,
    loops: i32,
    frame_count: u32,
    mut delay: impl FnMut(i32) -> u64,
) -> i32 {
    let mut new_frame = cur_frame;

    *cur_time = cur_time.wrapping_add(elapsed_time_ns);
    loop {
        let t = delay(new_frame);
        if *cur_time <= t {
            break;
        }

        *cur_time -= t;
        new_frame = new_frame.wrapping_add(1);
        if new_frame as u32 == frame_count {
            if loops == 0 || {
                *cur_loop = cur_loop.wrapping_add(1);
                *cur_loop < loops
            } {
                new_frame = 0;
            } else if *cur_loop == loops {
                new_frame = new_frame.wrapping_sub(1);
                break;
            }
        }
    }

    new_frame
}

/// The loop limit tick uses: libnsgif's `loop_max`, 0 (forever) from 0xFFFF.
pub fn effective_loops(loop_max: i32) -> i32 {
    if loop_max >= 0xFFFF { 0 } else { loop_max }
}

impl<H: Host> ImageFileEx<H> {
    /// `gs_image_file_ex_init`: counts the decoded size in `mem_usage`,
    /// which the Image Slide Show checks against its memory cap. The C
    /// passes `&image->mem_usage`; here the count is kept aside while
    /// `init_internal` resets `self`, and a failed load (which C frees and
    /// zeroes) counts nothing.
    pub fn init(&mut self, file: Option<&CStr>, alpha_mode: ImageAlphaMode) {
        let mut unused = ColorSpace(0);
        let mut mem_usage = 0;
        self.init_internal(file, Some(&mut mem_usage), &mut unused, alpha_mode);
        if self.loaded {
            self.mem_usage = mem_usage;
        }
    }

    /// `gs_image_file_ex_init_internal`.
    pub fn init_internal(
        &mut self,
        file: Option<&CStr>,
        mut mem_usage: Option<&mut u64>,
        space: &mut ColorSpace,
        alpha_mode: ImageAlphaMode,
    ) {
        *self = Self::default();

        let Some(file) = file else {
            return;
        };

        self.internal = Some(H::internal_new());

        if is_gif_path(file.to_bytes())
            && self.init_animated_gif(file, mem_usage.as_deref_mut(), alpha_mode)
        {
            return;
        }

        self.texture_data = H::decode_file(
            file,
            alpha_mode,
            &mut self.format,
            &mut self.cx,
            &mut self.cy,
            space,
        );

        if let Some(mem_usage) = mem_usage {
            let size = self
                .cx
                .wrapping_mul(self.cy)
                .wrapping_mul(format_bpp(self.format))
                / 8;
            *mem_usage = mem_usage.wrapping_add(u64::from(size));
        }

        self.loaded = self.texture_data.is_some();
        if !self.loaded {
            log_path::<H>(
                "gs_image_file_ex_init_internal",
                "Failed to load file '",
                file,
                "'",
            );
            self.free();
        }
    }

    #[expect(clippy::too_many_lines, reason = "one block per GIF header field")]
    fn init_animated_gif(
        &mut self,
        path: &CStr,
        mut mem_usage: Option<&mut u64>,
        alpha_mode: ImageAlphaMode,
    ) -> bool {
        const FUNCTION: &str = "init_animated_gif";
        // Allocated by init_internal just before.
        let Some(handle) = self.internal.as_mut() else {
            return false;
        };
        let internal = H::internal(handle);
        internal.gif = H::gif_create();
        let gif = internal.gif;

        let (size, complete) = match H::read_file(path) {
            FileRead::OpenFailed => {
                log_path::<H>(FUNCTION, "Failed to open file '", path, "'");
                return self.fail_gif();
            }
            FileRead::Read {
                data,
                size,
                complete,
            } => {
                internal.gif_data = Some(data);
                (size, complete)
            }
        };
        if !complete {
            log_path::<H>(FUNCTION, "Failed to fully read gif file '", path, "'.");
            return self.fail_gif();
        }

        loop {
            let result = H::gif_scan(gif, size, internal.gif_data);
            if result == NSGIF_OK {
                break;
            }
        }

        H::gif_complete(gif);

        let info = H::gif_info(gif);
        internal.gif_info = info;

        if info.width > 4096 || info.height > 4096 {
            Line::new()
                .push(FUNCTION.as_bytes())
                .push(b": Bad texture dimensions (")
                .push_i32(info.width as i32)
                .push(b"x")
                .push_i32(info.height as i32)
                .push(b") in '")
                .push(path.to_bytes())
                .push(b"'")
                .send::<H>();
            return self.fail_gif();
        }

        let max_size = u64::from(info.width)
            .wrapping_mul(u64::from(info.height))
            .wrapping_mul(u64::from(info.frame_count))
            .wrapping_mul(4);

        if full_decoded_gif_size(&info) as u64 != max_size {
            log_path::<H>(FUNCTION, "Gif '", path, "' overflowed maximum pointer size");
            return self.fail_gif();
        }

        self.is_animated_gif = info.frame_count > 1;
        if !self.is_animated_gif {
            H::gif_destroy(gif);
            H::free_data(internal.gif_data.take());
            return false;
        }

        let mut bitmap = None;
        let decode = |frame: u32, bitmap: &mut Option<H::Bitmap>| {
            let (result, decoded) = H::gif_decode(gif, frame);
            if result == NSGIF_OK {
                *bitmap = decoded;
            }
            result
        };
        decode(0, &mut bitmap);

        let cache_size = info.frame_count as usize * size_of::<usize>();
        if let Some(mem_usage) = mem_usage.as_deref_mut() {
            *mem_usage = mem_usage.wrapping_add(cache_size as u64);
        }
        internal.animation_frame_cache = Some(H::alloc_cache(info.frame_count));
        let data_size = full_decoded_gif_size(&info) as usize;
        if let Some(mem_usage) = mem_usage.as_deref_mut() {
            *mem_usage = mem_usage.wrapping_add(data_size as u64);
        }
        internal.animation_frame_data = Some(H::alloc_data(data_size));

        for i in 0..info.frame_count {
            if decode(i, &mut bitmap) != NSGIF_OK {
                Line::new()
                    .push(FUNCTION.as_bytes())
                    .push(b": Couldn't decode frame ")
                    .push_u32(i)
                    .push(b" of '")
                    .push(path.to_bytes())
                    .push(b"'")
                    .send::<H>();
            }
        }

        decode(0, &mut bitmap);

        internal.gif_frame_image = bitmap;

        self.cx = info.width;
        self.cy = info.height;
        self.format = GS_RGBA;

        if let Some(mem_usage) = mem_usage {
            let texels = 4 * self.cx as usize * self.cy as usize;
            *mem_usage = mem_usage
                .wrapping_add(texels as u64)
                .wrapping_add(size as u64);
        }

        premultiply_for::<H>(bitmap, self.cx as usize * self.cy as usize, alpha_mode);

        self.loaded = true;
        true
    }

    /// The `fail:` label of `init_animated_gif`: free, and report the file
    /// as a GIF so FFmpeg is not tried.
    fn fail_gif(&mut self) -> bool {
        if !self.loaded {
            // free() destroys the decoder only once loaded (#2032).
            if let Some(handle) = self.internal.as_mut() {
                let internal = H::internal(handle);
                H::gif_destroy(internal.gif.take());
            }
            self.free();
        }
        true
    }

    /// `gs_image_file_ex_free`.
    pub fn free(&mut self) {
        if self.loaded {
            if self.is_animated_gif
                && let Some(handle) = self.internal.as_mut()
            {
                let internal = H::internal(handle);
                H::gif_destroy(internal.gif);
                H::free_cache(internal.animation_frame_cache);
                H::free_data(internal.animation_frame_data);
            }

            H::texture_destroy(self.texture);
        }

        H::free_data(self.texture_data);
        if let Some(mut handle) = self.internal.take() {
            H::free_data(H::internal(&mut handle).gif_data);
            H::internal_free(handle);
        }
        *self = Self::default();
    }

    /// `gs_image_file_ex_init_texture`.
    pub fn init_texture(&mut self) {
        if !self.loaded {
            return;
        }

        if self.internal.is_none() {
            self.internal = Some(H::internal_new());
        }

        if self.is_animated_gif {
            let Some(handle) = self.internal.as_mut() else {
                return;
            };
            let internal = H::internal(handle);
            self.texture =
                H::texture_create_bitmap(self.cx, self.cy, self.format, internal.gif_frame_image);
        } else {
            self.texture = H::texture_create_data(self.cx, self.cy, self.format, self.texture_data);
            H::free_data(self.texture_data.take());
        }
    }

    fn decode_new_frame(&mut self, new_frame: i32, alpha_mode: ImageAlphaMode) {
        // The callers checked for it.
        let Some(handle) = self.internal.as_mut() else {
            return;
        };
        let internal = H::internal(handle);
        if !H::cache_has(internal.animation_frame_cache, new_frame) {
            // If looped, decode from frame 0.
            let last_frame = if new_frame < internal.last_decoded_frame {
                0
            } else {
                internal.last_decoded_frame.wrapping_add(1)
            };

            // Decode the missed frames.
            for i in last_frame..new_frame {
                if H::gif_decode(internal.gif, i as u32).0 != NSGIF_OK {
                    return;
                }
            }

            // Decode the frame itself.
            let (result, bitmap) = H::gif_decode(internal.gif, new_frame as u32);
            if result == NSGIF_OK {
                let area = internal.gif_info.width as usize * internal.gif_info.height as usize;
                let pos = (new_frame as usize).wrapping_mul(area).wrapping_mul(4);

                premultiply_for::<H>(bitmap, area, alpha_mode);

                internal.gif_frame_image = bitmap;

                H::cache_store(
                    internal.animation_frame_cache,
                    new_frame,
                    internal.animation_frame_data,
                    pos,
                    bitmap,
                    area * 4,
                );

                internal.last_decoded_frame = new_frame;
            }
        }

        self.cur_frame = new_frame;
    }

    /// `gs_image_file_ex_tick`.
    pub fn tick(&mut self, elapsed_time_ns: u64) -> bool {
        self.tick_internal(elapsed_time_ns, ImageAlphaMode::STRAIGHT)
    }

    fn tick_internal(&mut self, elapsed_time_ns: u64, alpha_mode: ImageAlphaMode) -> bool {
        if !self.is_animated_gif || !self.loaded {
            return false;
        }
        let Some(handle) = self.internal.as_mut() else {
            return false;
        };
        let internal = H::internal(handle);
        let gif = internal.gif;
        let info = internal.gif_info;

        let loops = effective_loops(info.loop_max);

        if loops == 0 || self.cur_loop < loops {
            let new_frame = calculate_new_frame(
                &mut self.cur_time,
                &mut self.cur_loop,
                self.cur_frame,
                elapsed_time_ns,
                loops,
                info.frame_count,
                |frame| frame_time_ns(H::gif_frame_delay(gif, frame)),
            );

            if new_frame != self.cur_frame {
                self.decode_new_frame(new_frame, alpha_mode);
                return true;
            }
        }

        false
    }

    /// `gs_image_file_ex_update_texture`.
    pub fn update_texture(&mut self) {
        self.update_texture_internal(ImageAlphaMode::STRAIGHT);
    }

    fn update_texture_internal(&mut self, alpha_mode: ImageAlphaMode) {
        if !self.is_animated_gif || !self.loaded {
            return;
        }
        let Some(handle) = self.internal.as_mut() else {
            return;
        };

        if !H::cache_has(H::internal(handle).animation_frame_cache, self.cur_frame) {
            self.decode_new_frame(self.cur_frame, alpha_mode);
        }

        let Some(handle) = self.internal.as_mut() else {
            return;
        };
        let internal = H::internal(handle);
        H::texture_set_image(
            self.texture,
            internal.animation_frame_cache,
            self.cur_frame,
            internal.gif_info.width.wrapping_mul(4),
        );
    }
}

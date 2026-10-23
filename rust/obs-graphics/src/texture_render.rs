//! The `gs_texrender_*` state machine from `libobs/graphics/texture-render.c.h`.
//!
//! A texrender owns one render-target texture and, when it has a zstencil
//! format, one zstencil buffer. `begin` sizes them, saves the current
//! render target, zstencil target and color space, pushes the viewport,
//! projection and matrix, and points rendering at its own target. `end`
//! restores what `begin` saved and pops the stacks. The graphics calls go
//! through [`Device`], in the same order as the C.
//!
//! Kept from the C (characterized, not endorsed):
//! - A failed texture create still stores the new size, so a second
//!   `begin` at that size does not retry and returns false.
//! - A second `begin` before `end` saves the texrender's own target as the
//!   previous one.
//! - `end` without `begin` restores the initial previous state (no target,
//!   no zstencil, sRGB) and marks the texrender rendered.
//! - The viewport size is the `u32` size cast to `i32`.

/// `enum gs_color_format`. Passed through to the device unchanged.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColorFormat(pub u32);

/// `enum gs_zstencil_format`. Only [`ZStencilFormat::NONE`] is special.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ZStencilFormat(pub u32);

impl ZStencilFormat {
    /// `GS_ZS_NONE`: no zstencil buffer.
    pub const NONE: Self = Self(0);
}

/// `enum gs_color_space`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColorSpace(pub u32);

impl ColorSpace {
    /// `GS_CS_SRGB`, what `begin` uses and the initial previous space.
    pub const SRGB: Self = Self(0);
}

/// `GS_RENDER_TARGET`, the flag of every texture a texrender creates.
pub const RENDER_TARGET: u32 = 1 << 2;

/// The graphics subsystem calls a texrender makes. Handles are `None` where
/// the C passes or gets `NULL`.
pub trait Device {
    type Texture: Copy + PartialEq;
    type ZStencil: Copy + PartialEq;

    /// `gs_texture_create(cx, cy, format, levels, NULL, flags)`.
    fn texture_create(
        &mut self,
        cx: u32,
        cy: u32,
        format: ColorFormat,
        levels: u32,
        flags: u32,
    ) -> Option<Self::Texture>;
    fn zstencil_create(
        &mut self,
        cx: u32,
        cy: u32,
        format: ZStencilFormat,
    ) -> Option<Self::ZStencil>;
    /// `gs_texture_destroy`, also called with `None`.
    fn texture_destroy(&mut self, texture: Option<Self::Texture>);
    /// `gs_zstencil_destroy`, also called with `None`.
    fn zstencil_destroy(&mut self, zstencil: Option<Self::ZStencil>);
    fn viewport_push(&mut self);
    fn viewport_pop(&mut self);
    fn projection_push(&mut self);
    fn projection_pop(&mut self);
    fn matrix_push(&mut self);
    fn matrix_pop(&mut self);
    fn matrix_identity(&mut self);
    fn render_target(&mut self) -> Option<Self::Texture>;
    fn zstencil_target(&mut self) -> Option<Self::ZStencil>;
    fn color_space(&mut self) -> ColorSpace;
    /// `gs_set_render_target_with_color_space`.
    fn set_render_target(
        &mut self,
        texture: Option<Self::Texture>,
        zstencil: Option<Self::ZStencil>,
        space: ColorSpace,
    );
    fn set_viewport(&mut self, x: i32, y: i32, cx: i32, cy: i32);
}

/// `struct gs_texture_render`.
pub struct TexRender<D: Device> {
    target: Option<D::Texture>,
    prev_target: Option<D::Texture>,
    zs: Option<D::ZStencil>,
    prev_zs: Option<D::ZStencil>,
    prev_space: ColorSpace,
    cx: u32,
    cy: u32,
    format: ColorFormat,
    zsformat: ZStencilFormat,
    rendered: bool,
}

impl<D: Device> TexRender<D> {
    /// `gs_texrender_create`: no buffers until the first `begin`.
    pub fn new(format: ColorFormat, zsformat: ZStencilFormat) -> Self {
        Self {
            target: None,
            prev_target: None,
            zs: None,
            prev_zs: None,
            prev_space: ColorSpace::SRGB,
            cx: 0,
            cy: 0,
            format,
            zsformat,
            rendered: false,
        }
    }

    /// `gs_texrender_destroy`: destroys the texture, then the zstencil,
    /// each even when absent.
    pub fn destroy(self, device: &mut D) {
        device.texture_destroy(self.target);
        device.zstencil_destroy(self.zs);
    }

    /// `texrender_resetbuffer`.
    fn reset_buffer(&mut self, device: &mut D, cx: u32, cy: u32) -> bool {
        device.texture_destroy(self.target);
        device.zstencil_destroy(self.zs);

        self.target = None;
        self.zs = None;
        self.cx = cx;
        self.cy = cy;

        self.target = device.texture_create(cx, cy, self.format, 1, RENDER_TARGET);
        if self.target.is_none() {
            return false;
        }

        if self.zsformat != ZStencilFormat::NONE {
            self.zs = device.zstencil_create(cx, cy, self.zsformat);
            if self.zs.is_none() {
                device.texture_destroy(self.target);
                self.target = None;
                return false;
            }
        }

        true
    }

    /// `gs_texrender_begin`: [`Self::begin_with_color_space`] in sRGB.
    pub fn begin(&mut self, device: &mut D, cx: u32, cy: u32) -> bool {
        self.begin_with_color_space(device, cx, cy, ColorSpace::SRGB)
    }

    /// `gs_texrender_begin_with_color_space`. False, with no graphics
    /// call, once rendered or for a zero size.
    pub fn begin_with_color_space(
        &mut self,
        device: &mut D,
        cx: u32,
        cy: u32,
        space: ColorSpace,
    ) -> bool {
        if self.rendered || cx == 0 || cy == 0 {
            return false;
        }

        if (self.cx != cx || self.cy != cy) && !self.reset_buffer(device, cx, cy) {
            return false;
        }

        if self.target.is_none() {
            return false;
        }

        device.viewport_push();
        device.projection_push();
        device.matrix_push();
        device.matrix_identity();

        self.prev_target = device.render_target();
        self.prev_zs = device.zstencil_target();
        self.prev_space = device.color_space();
        device.set_render_target(self.target, self.zs, space);

        // The C passes the uint32_t size to an int parameter.
        device.set_viewport(0, 0, self.cx as i32, self.cy as i32);

        true
    }

    /// `gs_texrender_end`.
    pub fn end(&mut self, device: &mut D) {
        device.set_render_target(self.prev_target, self.prev_zs, self.prev_space);

        device.matrix_pop();
        device.projection_pop();
        device.viewport_pop();

        self.rendered = true;
    }

    /// `gs_texrender_reset`: allows the next `begin`.
    pub fn reset(&mut self) {
        self.rendered = false;
    }

    /// `gs_texrender_get_texture`.
    pub fn texture(&self) -> Option<D::Texture> {
        self.target
    }

    /// `gs_texrender_get_format`.
    pub fn format(&self) -> ColorFormat {
        self.format
    }
}

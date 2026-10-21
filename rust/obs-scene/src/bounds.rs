//! Safe core of `obs_bounds_fit_scale`.

use obs_graphics::vec2::Vec2;

/// `enum obs_bounds_type` in `libobs/obs.h`, same numeric order.
///
/// [`BoundsType::Other`] is a value outside that enum. C matches no branch
/// and leaves the scale unchanged, the same as [`BoundsType::None`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BoundsType {
    None = 0,
    Stretch = 1,
    ScaleInner = 2,
    ScaleOuter = 3,
    ScaleToWidth = 4,
    ScaleToHeight = 5,
    MaxOnly = 6,
    Other = -1,
}

impl BoundsType {
    /// The C enumerator, or [`BoundsType::Other`] for any other `int`.
    #[must_use]
    pub fn from_c(value: i32) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Stretch,
            2 => Self::ScaleInner,
            3 => Self::ScaleOuter,
            4 => Self::ScaleToWidth,
            5 => Self::ScaleToHeight,
            6 => Self::MaxOnly,
            _ => Self::Other,
        }
    }
}

/// `vec2_mulf`: both components, including a negative scale.
fn mulf(scale: Vec2, factor: f32) -> Vec2 {
    Vec2::new(scale.x * factor, scale.y * factor)
}

/// Fit `scale` for a `cx` by `cy` source drawn into `bounds`.
///
/// A zero size on the axis a fit would divide by leaves `scale` unchanged,
/// so an empty source stays finite. `vec2_mulf` stays the header inline;
/// this multiplies both components the same way.
#[must_use]
pub fn bounds_fit_scale(kind: BoundsType, bounds: Vec2, cx: u32, cy: u32, mut scale: Vec2) -> Vec2 {
    let width = cx as f32 * scale.x.abs();
    let height = cy as f32 * scale.y.abs();
    let mut bounds_type = kind;

    // Past the box only. A scaled size equal to the box stays put.
    // The outer flip below still looks at `kind`, so max-only does not flip.
    if kind == BoundsType::MaxOnly && (width > bounds.x || height > bounds.y) {
        bounds_type = BoundsType::ScaleInner;
    }

    match bounds_type {
        BoundsType::ScaleInner | BoundsType::ScaleOuter => {
            let item_aspect = width / height;
            let bounds_aspect = bounds.x / bounds.y;
            let mut use_width = bounds_aspect < item_aspect;
            if kind == BoundsType::ScaleOuter {
                use_width = !use_width;
            }
            let size = if use_width { width } else { height };
            if size > 0.0 {
                let edge = if use_width { bounds.x } else { bounds.y };
                scale = mulf(scale, edge / size);
            }
        }
        BoundsType::ScaleToWidth => {
            if width > 0.0 {
                scale = mulf(scale, bounds.x / width);
            }
        }
        BoundsType::ScaleToHeight => {
            if height > 0.0 {
                scale = mulf(scale, bounds.y / height);
            }
        }
        BoundsType::Stretch => {
            if cx != 0 {
                scale.x = (bounds.x / cx as f32).copysign(scale.x);
            }
            if cy != 0 {
                scale.y = (bounds.y / cy as f32).copysign(scale.y);
            }
        }
        BoundsType::MaxOnly | BoundsType::None | BoundsType::Other => {}
    }
    scale
}

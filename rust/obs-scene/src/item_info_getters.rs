//! Position, bounds and transform-info reads: the stored values, or the
//! existing C canvas conversions for relative coordinates.

/// A canvas conversion the C keeps (`obs-scene.c.h`), named by the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conversion {
    Position,
    Scale,
    Bounds,
}

/// The fields of `struct obs_transform_info`, floats as raw bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransformBits {
    pub pos: [u32; 2],
    pub rot: u32,
    pub scale: [u32; 2],
    pub alignment: u32,
    pub bounds_type: u32,
    pub bounds_alignment: u32,
    pub bounds: [u32; 2],
    pub crop_to_bounds: bool,
}

/// Position or bounds: a missing item leaves output untouched. Relative
/// values use the callback while the caller holds the parent's video mutex.
pub fn read_vec2(
    value: Option<([u32; 2], bool)>,
    relative: impl FnOnce() -> [u32; 2],
    output: &mut [u32; 2],
) {
    crate::item_scale_getter::read_scale(value, relative, output);
}

/// The info written for a stored item and its `absolute_coordinates` flag;
/// `None` (a missing item or output) writes nothing. Relative coordinates
/// convert position, scale and bounds, once each and in that order.
pub fn read_info(
    value: Option<(TransformBits, bool)>,
    mut convert: impl FnMut(Conversion) -> [u32; 2],
) -> Option<TransformBits> {
    let (stored, absolute) = value?;
    if absolute {
        return Some(stored);
    }
    let pos = convert(Conversion::Position);
    let scale = convert(Conversion::Scale);
    let bounds = convert(Conversion::Bounds);
    Some(TransformBits {
        pos,
        scale,
        bounds,
        ..stored
    })
}

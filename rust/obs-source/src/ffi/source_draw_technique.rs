//! `enum obs_source_draw_technique` (`libobs/obs-source-draw-technique.h`),
//! which the deinterlace draw shims (`source_deinterlace_draw`) and the
//! source draw plan shims (`source_draw_plan`) return. Its values run from 0
//! to 5, so the C's enum type is `unsigned int` or `int`, and every value
//! fits both; [`crate::draw_technique::Technique::value`] gives it.

/// `enum obs_source_draw_technique`.
#[allow(non_camel_case_types)]
pub type obs_source_draw_technique = u32;

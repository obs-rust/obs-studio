//! `gs_effect` and the param, technique, and pass structs match
//! `libobs/graphics/effect.h`.

use core::mem::{align_of, offset_of, size_of};

use obs_c_oracle::effect_lookups as c;
use obs_graphics::ffi::effect_lookups::{
    gs_effect, gs_effect_param, gs_effect_param_info, gs_effect_pass, gs_effect_technique,
};

macro_rules! layout {
    ($ty:ty, $size:path, $align:path, $($field:ident = $off:path),+ $(,)?) => {{
        assert_eq!(size_of::<$ty>(), $size());
        assert_eq!(align_of::<$ty>(), $align());
        $(
            assert_eq!(offset_of!($ty, $field), $off(), stringify!($field));
        )+
    }};
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn effect_layouts_match_c() {
    // SAFETY: the oracle layout functions take no arguments and only return constants.
    unsafe {
        layout!(
            gs_effect_param,
            c::oracle_effect_param_size,
            c::oracle_effect_param_align,
            name = c::oracle_effect_param_offset_name,
            section = c::oracle_effect_param_offset_section,
            type_ = c::oracle_effect_param_offset_type,
            changed = c::oracle_effect_param_offset_changed,
            cur_val = c::oracle_effect_param_offset_cur_val,
            default_val = c::oracle_effect_param_offset_default_val,
            effect = c::oracle_effect_param_offset_effect,
            next_sampler = c::oracle_effect_param_offset_next_sampler,
            annotations = c::oracle_effect_param_offset_annotations,
        );
        layout!(
            gs_effect_pass,
            c::oracle_effect_pass_size,
            c::oracle_effect_pass_align,
            name = c::oracle_effect_pass_offset_name,
            section = c::oracle_effect_pass_offset_section,
            vertshader = c::oracle_effect_pass_offset_vertshader,
            pixelshader = c::oracle_effect_pass_offset_pixelshader,
            vertshader_params = c::oracle_effect_pass_offset_vertshader_params,
            pixelshader_params = c::oracle_effect_pass_offset_pixelshader_params,
        );
        layout!(
            gs_effect_technique,
            c::oracle_effect_technique_size,
            c::oracle_effect_technique_align,
            name = c::oracle_effect_technique_offset_name,
            section = c::oracle_effect_technique_offset_section,
            effect = c::oracle_effect_technique_offset_effect,
            passes = c::oracle_effect_technique_offset_passes,
        );
        layout!(
            gs_effect,
            c::oracle_effect_size,
            c::oracle_effect_align,
            processing = c::oracle_effect_offset_processing,
            cached = c::oracle_effect_offset_cached,
            effect_path = c::oracle_effect_offset_effect_path,
            effect_dir = c::oracle_effect_offset_effect_dir,
            params = c::oracle_effect_offset_params,
            techniques = c::oracle_effect_offset_techniques,
            cur_technique = c::oracle_effect_offset_cur_technique,
            cur_pass = c::oracle_effect_offset_cur_pass,
            view_proj = c::oracle_effect_offset_view_proj,
            world = c::oracle_effect_offset_world,
            scale = c::oracle_effect_offset_scale,
            graphics = c::oracle_effect_offset_graphics,
            next = c::oracle_effect_offset_next,
            loop_pass = c::oracle_effect_offset_loop_pass,
            looping = c::oracle_effect_offset_looping,
        );
        layout!(
            gs_effect_param_info,
            c::oracle_effect_param_info_size,
            c::oracle_effect_param_info_align,
            name = c::oracle_effect_param_info_offset_name,
            type_ = c::oracle_effect_param_info_offset_type,
        );
    }
}

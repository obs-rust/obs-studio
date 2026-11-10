#include "libobs/build/libobs.pch.h"

gs_effect_t *obs_base_effect_default_field(const struct obs_core *core);
gs_effect_t *obs_base_effect_default_rect_field(const struct obs_core *core);
gs_effect_t *obs_base_effect_opaque_field(const struct obs_core *core);
gs_effect_t *obs_base_effect_solid_field(const struct obs_core *core);
gs_effect_t *obs_base_effect_bicubic_field(const struct obs_core *core);
gs_effect_t *obs_base_effect_lanczos_field(const struct obs_core *core);
gs_effect_t *obs_base_effect_bilinear_lowres_field(const struct obs_core *core);
gs_effect_t *obs_base_effect_premultiplied_alpha_field(const struct obs_core *core);
gs_effect_t *obs_base_effect_repeat_field(const struct obs_core *core);
gs_effect_t *obs_base_effect_area_field(const struct obs_core *core);

gs_effect_t *obs_base_effect_default_field(const struct obs_core *core)
{
	return core->video.default_effect;
}

gs_effect_t *obs_base_effect_default_rect_field(const struct obs_core *core)
{
	return core->video.default_rect_effect;
}

gs_effect_t *obs_base_effect_opaque_field(const struct obs_core *core)
{
	return core->video.opaque_effect;
}

gs_effect_t *obs_base_effect_solid_field(const struct obs_core *core)
{
	return core->video.solid_effect;
}

gs_effect_t *obs_base_effect_bicubic_field(const struct obs_core *core)
{
	return core->video.bicubic_effect;
}

gs_effect_t *obs_base_effect_lanczos_field(const struct obs_core *core)
{
	return core->video.lanczos_effect;
}

gs_effect_t *obs_base_effect_bilinear_lowres_field(const struct obs_core *core)
{
	return core->video.bilinear_lowres_effect;
}

gs_effect_t *obs_base_effect_premultiplied_alpha_field(const struct obs_core *core)
{
	return core->video.premultiplied_alpha_effect;
}

gs_effect_t *obs_base_effect_repeat_field(const struct obs_core *core)
{
	return core->video.repeat_effect;
}

gs_effect_t *obs_base_effect_area_field(const struct obs_core *core)
{
	return core->video.area_effect;
}

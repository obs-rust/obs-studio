#include "libobs/build/libobs.pch.h"

/* obs-source.c.h's filter draws are static there; this extraction reaches
 * them through their hidden wrappers (no EXPORT), under the original names,
 * scoped to this file. */
void obs_source_process_filter_end_render_bypass(obs_source_t *target, gs_effect_t *effect, const char *tech_name);
void obs_source_process_filter_end_render_tex(gs_texture_t *tex, gs_effect_t *effect, uint32_t width, uint32_t height,
					      const char *tech_name);
#define render_filter_bypass obs_source_process_filter_end_render_bypass
#define render_filter_tex obs_source_process_filter_end_render_tex

void obs_source_process_filter_tech_end(obs_source_t *filter, gs_effect_t *effect, uint32_t width, uint32_t height,
					const char *tech_name)
{
	obs_source_t *target, *parent;
	gs_texture_t *texture;
	uint32_t filter_flags;

	if (!filter)
		return;

	const bool filter_bypass_active = filter->filter_bypass_active;
	filter->filter_bypass_active = false;

	target = obs_filter_get_target(filter);
	parent = obs_filter_get_parent(filter);

	if (obs_source_render_route_filter(target != NULL, parent != NULL) != OBS_SOURCE_RENDER_ROUTE_FILTER_READY)
		return;

	filter_flags = filter->info.output_flags;

	const bool previous = gs_set_linear_srgb(obs_source_render_route_tech_end_linear_srgb(filter_flags));

	const char *tech = obs_source_render_route_tech_name(tech_name);

	switch (obs_source_render_route_tech_end(filter_bypass_active)) {
	case OBS_SOURCE_RENDER_ROUTE_TECH_END_BYPASS:
		render_filter_bypass(target, effect, tech);
		break;
	case OBS_SOURCE_RENDER_ROUTE_TECH_END_TEXTURE:
		texture = gs_texrender_get_texture(filter->filter_texrender);
		if (texture) {
			render_filter_tex(texture, effect, width, height, tech);
		}
		break;
	}

	gs_set_linear_srgb(previous);
}

void obs_source_process_filter_end(obs_source_t *filter, gs_effect_t *effect, uint32_t width, uint32_t height)
{
	if (!obs_ptr_valid(filter, "obs_source_process_filter_end"))
		return;

	obs_source_process_filter_tech_end(filter, effect, width, height, "Draw");
}

#undef render_filter_tex
#undef render_filter_bypass

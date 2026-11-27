#include "libobs/build/libobs.pch.h"

/* can_bypass(): gs_get_color_space() is called only when the other
 * conditions hold, as before. */
static inline bool can_bypass(obs_source_t *target, obs_source_t *parent, uint32_t filter_flags, uint32_t parent_flags,
			      enum obs_allow_direct_render allow_direct, enum gs_color_space space)
{
	const bool reads_space = obs_source_render_route_bypass_reads_space(target == parent, (int)allow_direct,
									    filter_flags, parent_flags);
	return obs_source_render_route_bypass(target == parent, (int)allow_direct, filter_flags, parent_flags,
					      (int)space, reads_space ? (int)gs_get_color_space() : (int)GS_CS_SRGB);
}

bool obs_source_process_filter_begin(obs_source_t *filter, enum gs_color_format format,
				     enum obs_allow_direct_render allow_direct)
{
	return obs_source_process_filter_begin_with_color_space(filter, format, GS_CS_SRGB, allow_direct);
}

bool obs_source_process_filter_begin_with_color_space(obs_source_t *filter, enum gs_color_format format,
						      enum gs_color_space space,
						      enum obs_allow_direct_render allow_direct)
{
	obs_source_t *target, *parent;
	uint32_t filter_flags, parent_flags;
	int cx, cy;

	if (!obs_ptr_valid(filter, "obs_source_process_filter_begin_with_color_space"))
		return false;

	filter->filter_bypass_active = false;

	target = obs_filter_get_target(filter);
	parent = obs_filter_get_parent(filter);

	switch (obs_source_render_route_filter(target != NULL, parent != NULL)) {
	case OBS_SOURCE_RENDER_ROUTE_FILTER_NO_TARGET:
		blog(LOG_INFO, "filter '%s' being processed with no target!", filter->context.name);
		return false;
	case OBS_SOURCE_RENDER_ROUTE_FILTER_NO_PARENT:
		blog(LOG_INFO, "filter '%s' being processed with no parent!", filter->context.name);
		return false;
	case OBS_SOURCE_RENDER_ROUTE_FILTER_READY:
		break;
	}

	filter_flags = filter->info.output_flags;
	parent_flags = parent->info.output_flags;
	cx = (int)obs_source_base_width(target);
	cy = (int)obs_source_base_height(target);

	filter->allow_direct = allow_direct;

	/* if the parent does not use any custom effects, and this is the last
	 * filter in the chain for the parent, then render the parent directly
	 * using the filter effect instead of rendering to texture to reduce
	 * the total number of passes */
	if (can_bypass(target, parent, filter_flags, parent_flags, allow_direct, space)) {
		filter->filter_bypass_active = true;
		return true;
	}

	if (obs_source_render_route_size_skip(cx, cy)) {
		obs_source_skip_video_filter(filter);
		return false;
	}

	if (filter->filter_texrender && obs_source_render_route_texrender_stale(
						(int)gs_texrender_get_format(filter->filter_texrender), (int)format)) {
		gs_texrender_destroy(filter->filter_texrender);
		filter->filter_texrender = NULL;
	}

	if (!filter->filter_texrender) {
		filter->filter_texrender = gs_texrender_create(format, GS_ZS_NONE);
	}

	if (gs_texrender_begin_with_color_space(filter->filter_texrender, (uint32_t)cx, (uint32_t)cy, space)) {
		gs_blend_state_push();
		gs_blend_function_separate(GS_BLEND_SRCALPHA, GS_BLEND_INVSRCALPHA, GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);

		struct vec4 clear_color;

		vec4_zero(&clear_color);
		gs_clear(GS_CLEAR_COLOR, &clear_color, 0.0f, 0);
		gs_ortho(0.0f, (float)cx, 0.0f, (float)cy, -100.0f, 100.0f);

		if (obs_source_render_route_filter_render(target == parent, parent_flags) ==
		    OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT)
			obs_source_default_render(target);
		else
			obs_source_video_render(target);

		gs_blend_state_pop();

		gs_texrender_end(filter->filter_texrender);
	}
	return true;
}

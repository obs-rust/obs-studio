#include "libobs/build/libobs.pch.h"

/* obs-source.c.h's main render and async video draws are static there; this
 * extraction reaches them through their hidden wrappers (no EXPORT), under
 * the original names, scoped to this file. deinterlacing_enabled() is a
 * file-local copy of obs-source.c.h's. */
void obs_source_skip_video_filter_main_render(obs_source_t *source);
void obs_source_skip_video_filter_async_video(obs_source_t *source);
#define obs_source_main_render obs_source_skip_video_filter_main_render
#define obs_source_render_async_video obs_source_skip_video_filter_async_video

static inline bool skip_video_filter_deinterlacing_enabled(const struct obs_source *source)
{
	return source->deinterlace_mode != OBS_DEINTERLACE_MODE_DISABLE;
}
#define deinterlacing_enabled skip_video_filter_deinterlacing_enabled

void obs_source_skip_video_filter(obs_source_t *filter)
{
	obs_source_t *target, *parent;
	uint32_t parent_flags;

	if (!obs_ptr_valid(filter, "obs_source_skip_video_filter"))
		return;

	target = obs_filter_get_target(filter);
	parent = obs_filter_get_parent(filter);

	/* A filter removed while it renders has no parent or target any
	 * more: draw nothing this frame, as obs_source_process_filter_begin
	 * does. */
	if (obs_source_render_route_filter(target != NULL, parent != NULL) != OBS_SOURCE_RENDER_ROUTE_FILTER_READY)
		return;

	parent_flags = parent->info.output_flags;

	/* each READ_ step reads one value of the target, in the old order */
	enum obs_source_render_route_draw route = obs_source_render_route_skip(target == parent, parent_flags);
	if (route == OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER)
		route = obs_source_render_route_draw_next(route, target->info.video_render != NULL);
	if (route == OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING)
		route = obs_source_render_route_draw_next(route, deinterlacing_enabled(target));

	switch (route) {
	case OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT:
		obs_source_default_render(target);
		break;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN:
		obs_source_main_render(target);
		break;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE:
		deinterlace_render(target);
		break;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC:
		obs_source_render_async_video(target);
		break;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET:
		obs_source_video_render(target);
		break;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING:
		/* not a draw of a skipped filter */
		break;
	}
}

#undef deinterlacing_enabled
#undef obs_source_render_async_video
#undef obs_source_main_render

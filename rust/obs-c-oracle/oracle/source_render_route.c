/* Test-only oracle: the original libobs/obs-source-render-route.c.h,
 * unmodified, with its 23 functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_render_route_filter oracle_obs_source_render_route_filter
#define obs_source_render_route_video_reads_flags oracle_obs_source_render_route_video_reads_flags
#define obs_source_render_route_video_none oracle_obs_source_render_route_video_none
#define obs_source_render_route_async_reads_flags oracle_obs_source_render_route_async_reads_flags
#define obs_source_render_route_async_reads_rendering_filter oracle_obs_source_render_route_async_reads_rendering_filter
#define obs_source_render_route_async_update oracle_obs_source_render_route_async_update
#define obs_source_render_route_reads_enabled oracle_obs_source_render_route_reads_enabled
#define obs_source_render_route_inactive oracle_obs_source_render_route_inactive
#define obs_source_render_route_draw oracle_obs_source_render_route_draw
#define obs_source_render_route_draw_next oracle_obs_source_render_route_draw_next
#define obs_source_render_route_main_custom_draw oracle_obs_source_render_route_main_custom_draw
#define obs_source_render_route_main_srgb_aware oracle_obs_source_render_route_main_srgb_aware
#define obs_source_render_route_main_reads_filters oracle_obs_source_render_route_main_reads_filters
#define obs_source_render_route_main_default_effect oracle_obs_source_render_route_main_default_effect
#define obs_source_render_route_bypass_reads_space oracle_obs_source_render_route_bypass_reads_space
#define obs_source_render_route_bypass oracle_obs_source_render_route_bypass
#define obs_source_render_route_size_skip oracle_obs_source_render_route_size_skip
#define obs_source_render_route_texrender_stale oracle_obs_source_render_route_texrender_stale
#define obs_source_render_route_filter_render oracle_obs_source_render_route_filter_render
#define obs_source_render_route_tech_end_linear_srgb oracle_obs_source_render_route_tech_end_linear_srgb
#define obs_source_render_route_tech_name oracle_obs_source_render_route_tech_name
#define obs_source_render_route_tech_end oracle_obs_source_render_route_tech_end
#define obs_source_render_route_skip oracle_obs_source_render_route_skip

#include "obs-source-render-route.c.h"

#undef obs_source_render_route_skip
#undef obs_source_render_route_tech_end
#undef obs_source_render_route_tech_name
#undef obs_source_render_route_tech_end_linear_srgb
#undef obs_source_render_route_filter_render
#undef obs_source_render_route_texrender_stale
#undef obs_source_render_route_size_skip
#undef obs_source_render_route_bypass
#undef obs_source_render_route_bypass_reads_space
#undef obs_source_render_route_main_default_effect
#undef obs_source_render_route_main_reads_filters
#undef obs_source_render_route_main_srgb_aware
#undef obs_source_render_route_main_custom_draw
#undef obs_source_render_route_draw_next
#undef obs_source_render_route_draw
#undef obs_source_render_route_inactive
#undef obs_source_render_route_reads_enabled
#undef obs_source_render_route_async_update
#undef obs_source_render_route_async_reads_rendering_filter
#undef obs_source_render_route_async_reads_flags
#undef obs_source_render_route_video_none
#undef obs_source_render_route_video_reads_flags
#undef obs_source_render_route_filter

/* The enum values and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/source_render_route.rs; INT64_MIN past the end. The
 * renames above also renamed the three enum tags that share their function's
 * name, so the size table names the renamed tags. */
int64_t oracle_source_render_route_value(size_t which);

int64_t oracle_source_render_route_value(size_t which)
{
	const int64_t values[] = {
		(int64_t)OBS_SOURCE_TYPE_INPUT,
		(int64_t)OBS_SOURCE_TYPE_FILTER,
		(int64_t)OBS_SOURCE_TYPE_TRANSITION,
		(int64_t)OBS_SOURCE_TYPE_SCENE,
		(int64_t)OBS_SOURCE_VIDEO,
		(int64_t)OBS_SOURCE_AUDIO,
		(int64_t)OBS_SOURCE_ASYNC,
		(int64_t)OBS_SOURCE_CUSTOM_DRAW,
		(int64_t)OBS_SOURCE_CAP_DISABLED,
		(int64_t)OBS_SOURCE_SRGB,
		(int64_t)OBS_NO_DIRECT_RENDERING,
		(int64_t)OBS_ALLOW_DIRECT_RENDERING,
		(int64_t)GS_CS_SRGB,
		(int64_t)GS_CS_SRGB_16F,
		(int64_t)GS_CS_709_EXTENDED,
		(int64_t)GS_CS_709_SCRGB,
		(int64_t)GS_RG16,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_FILTER_READY,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_FILTER_NO_TARGET,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_FILTER_NO_PARENT,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_TECH_END_BYPASS,
		(int64_t)OBS_SOURCE_RENDER_ROUTE_TECH_END_TEXTURE,
		(int64_t)sizeof(enum obs_source_type),
		(int64_t)sizeof(enum obs_allow_direct_render),
		(int64_t)sizeof(enum gs_color_space),
		(int64_t)sizeof(enum gs_color_format),
		(int64_t)sizeof(enum oracle_obs_source_render_route_filter),
		(int64_t)sizeof(enum oracle_obs_source_render_route_draw),
		(int64_t)sizeof(enum oracle_obs_source_render_route_tech_end),
		(int64_t)sizeof(int),
		(int64_t)sizeof(bool),
		(int64_t)sizeof(uint32_t),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}

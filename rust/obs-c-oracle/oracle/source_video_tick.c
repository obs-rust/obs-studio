/* Test-only oracle: the original libobs/obs-source-video-tick.c.h,
 * unmodified, with its seven functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_video_tick_transition oracle_obs_source_video_tick_transition
#define obs_source_video_tick_async oracle_obs_source_video_tick_async
#define obs_source_video_tick_media oracle_obs_source_video_tick_media
#define obs_source_video_tick_deferred_update oracle_obs_source_video_tick_deferred_update
#define obs_source_video_tick_edge oracle_obs_source_video_tick_edge
#define obs_source_video_tick_calls_tick oracle_obs_source_video_tick_calls_tick
#define obs_source_video_tick_async_route oracle_obs_source_video_tick_async_route

#include "obs-source-video-tick.c.h"

#undef obs_source_video_tick_async_route
#undef obs_source_video_tick_calls_tick
#undef obs_source_video_tick_edge
#undef obs_source_video_tick_deferred_update
#undef obs_source_video_tick_media
#undef obs_source_video_tick_async
#undef obs_source_video_tick_transition

/* The values and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/source_video_tick.rs; INT64_MIN past the end. The
 * renames above also renamed the enum tags that share a function's name,
 * enum obs_source_video_tick_edge and enum
 * obs_source_video_tick_async_route, so their sizes are taken under the
 * oracle_ tag. */
int64_t oracle_source_video_tick_value(size_t which);

int64_t oracle_source_video_tick_value(size_t which)
{
	const int64_t values[] = {
		OBS_SOURCE_VIDEO_TICK_TYPE_TRANSITION,
		OBS_SOURCE_VIDEO_TICK_ASYNC,
		OBS_SOURCE_VIDEO_TICK_CONTROLLABLE_MEDIA,
		OBS_SOURCE_VIDEO_TICK_EDGE_NONE,
		OBS_SOURCE_VIDEO_TICK_EDGE_ON,
		OBS_SOURCE_VIDEO_TICK_EDGE_OFF,
		OBS_SOURCE_VIDEO_TICK_ASYNC_CLOSEST,
		OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE,
		(int64_t)sizeof(enum oracle_obs_source_video_tick_edge),
		(int64_t)sizeof(enum oracle_obs_source_video_tick_async_route),
		(int64_t)sizeof(long),
	};

	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}

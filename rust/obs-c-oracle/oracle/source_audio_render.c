/* Test-only oracle: the original libobs/obs-source-audio-render.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims. */
#define obs_source_audio_render_route oracle_obs_source_audio_render_route
#define obs_source_audio_render_custom_clear_before oracle_obs_source_audio_render_custom_clear_before
#define obs_source_audio_render_custom_ts oracle_obs_source_audio_render_custom_ts
#define obs_source_audio_render_custom_pending oracle_obs_source_audio_render_custom_pending
#define obs_source_audio_render_custom_unmixed oracle_obs_source_audio_render_custom_unmixed
#define obs_source_audio_render_custom_in_tick oracle_obs_source_audio_render_custom_in_tick
#define obs_source_audio_render_custom_clear_after oracle_obs_source_audio_render_custom_clear_after
#define obs_source_audio_render_tick_unset oracle_obs_source_audio_render_tick_unset
#define obs_source_audio_render_tick_short oracle_obs_source_audio_render_tick_short
#define obs_source_audio_render_tick_stop oracle_obs_source_audio_render_tick_stop
#define obs_source_audio_render_tick_copy oracle_obs_source_audio_render_tick_copy
#define obs_source_audio_render_tick_volume oracle_obs_source_audio_render_tick_volume
#define obs_source_audio_render_tick_clear_main oracle_obs_source_audio_render_tick_clear_main

#include "obs-source-audio-render.c.h"

#undef obs_source_audio_render_route
#undef obs_source_audio_render_custom_clear_before
#undef obs_source_audio_render_custom_ts
#undef obs_source_audio_render_custom_pending
#undef obs_source_audio_render_custom_unmixed
#undef obs_source_audio_render_custom_in_tick
#undef obs_source_audio_render_custom_clear_after
#undef obs_source_audio_render_tick_unset
#undef obs_source_audio_render_tick_short
#undef obs_source_audio_render_tick_stop
#undef obs_source_audio_render_tick_copy
#undef obs_source_audio_render_tick_volume
#undef obs_source_audio_render_tick_clear_main

/* The header's enum values and size, for the Rust side. */
unsigned oracle_source_audio_render_route_value(int which);
unsigned oracle_source_audio_render_route_size(void);

unsigned oracle_source_audio_render_route_value(int which)
{
	switch (which) {
	case 0:
		return OBS_SOURCE_AUDIO_RENDER_PENDING;
	case 1:
		return OBS_SOURCE_AUDIO_RENDER_CUSTOM;
	default:
		return OBS_SOURCE_AUDIO_RENDER_TICK;
	}
}

unsigned oracle_source_audio_render_route_size(void)
{
	/* The enum shares its tag with the route function, which the rename above
	 * also renamed, so take the size of what the function returns. */
	return (unsigned)sizeof(oracle_obs_source_audio_render_route(false, false, false));
}

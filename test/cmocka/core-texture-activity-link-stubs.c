/* Unrelated standalone Rust tests do not own core texture fixtures. */
#include <stdlib.h>
#include "obs-internal.h"

const struct obs_core *obs_core_texture_activity_core(void)
{
	abort();
}

obs_canvas_t *const *obs_core_texture_activity_canvas(const struct obs_core *core)
{
	(void)core;
	abort();
}

struct obs_core_video_mix *const *obs_core_texture_activity_mix(const obs_canvas_t *canvas)
{
	(void)canvas;
	abort();
}

struct obs_core_texture_activity_fields obs_core_texture_activity_fields(const struct obs_core_video_mix *mix)
{
	(void)mix;
	abort();
}

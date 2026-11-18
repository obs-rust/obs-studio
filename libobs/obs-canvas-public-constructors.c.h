#include "libobs/build/libobs.pch.h"

/* Defined later by obs-canvas.c.h in the core-source unity group. */
static obs_canvas_t *obs_canvas_create_internal(const char *name, const char *uuid, struct obs_video_info *ovi,
						uint32_t flags, bool private);

obs_canvas_t *obs_canvas_create(const char *name, struct obs_video_info *ovi, uint32_t flags)
{
	flags = obs_canvas_rules_public_flags(flags); /* Prevent user from creating a MAIN or removed canvas. */
	return obs_canvas_create_internal(name, NULL, ovi, flags, false);
}

obs_canvas_t *obs_canvas_create_private(const char *name, struct obs_video_info *ovi, uint32_t flags)
{
	flags = obs_canvas_rules_public_flags(flags); /* Prevent user from creating a MAIN or removed canvas. */
	return obs_canvas_create_internal(name, NULL, ovi, flags, true);
}

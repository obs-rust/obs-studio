#include "libobs/build/libobs.pch.h"

/* Defined later by obs-canvas.c.h in the core-source unity group. */
static obs_canvas_t *obs_canvas_create_internal(const char *name, const char *uuid, struct obs_video_info *ovi,
						uint32_t flags, bool private);

obs_data_t *obs_save_canvas(obs_canvas_t *canvas)
{
	if (!obs_canvas_rules_saves(canvas->flags))
		return NULL;

	obs_data_t *canvas_data = obs_data_create();

	obs_data_set_string(canvas_data, "name", canvas->context.name);
	obs_data_set_string(canvas_data, "uuid", canvas->context.uuid);
	obs_data_set_bool(canvas_data, "private", canvas->context.private);
	obs_data_set_int(canvas_data, "flags", canvas->flags);

	return canvas_data;
}

obs_canvas_t *obs_load_canvas(obs_data_t *data)
{
	const char *name = obs_data_get_string(data, "name");
	const char *uuid = obs_data_get_string(data, "uuid");
	const bool private = obs_data_get_bool(data, "private");
	uint32_t flags = (uint32_t)obs_data_get_int(data, "flags");

	flags = obs_canvas_rules_public_flags(flags); /* Prevent user from creating a MAIN or removed canvas. */
	return obs_canvas_create_internal(name, uuid, NULL, flags, private);
}

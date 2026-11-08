/* Original C getters and real obs-internal.h layouts. No exclusions. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_canvas_get_name oracle_obs_canvas_get_name
#define obs_canvas_get_uuid oracle_obs_canvas_get_uuid
#define obs_canvas_get_signal_handler oracle_obs_canvas_get_signal_handler
#define obs_canvas_has_video oracle_obs_canvas_has_video
#include "obs-canvas-readonly-getters.c.h"
#undef obs_canvas_get_name
#undef obs_canvas_get_uuid
#undef obs_canvas_get_signal_handler
#undef obs_canvas_has_video
#include "canvas_readonly_fields.inc"

struct canvas_readonly_fixture {
	struct obs_canvas canvas;
	struct obs_canvas before;
	struct obs_core_video_mix mix;
};

void oracle_canvas_readonly_layout(size_t *layout);
void oracle_canvas_readonly_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_canvas_readonly_fields);
	layout[1] = _Alignof(struct obs_canvas_readonly_fields);
	layout[2] = offsetof(struct obs_canvas_readonly_fields, name);
	layout[3] = offsetof(struct obs_canvas_readonly_fields, uuid);
	layout[4] = offsetof(struct obs_canvas_readonly_fields, signals);
	layout[5] = offsetof(struct obs_canvas_readonly_fields, mix);
}

void *oracle_canvas_readonly_new(char *name, char *uuid, void *signals, bool mixed);
void *oracle_canvas_readonly_new(char *name, char *uuid, void *signals, bool mixed)
{
	struct canvas_readonly_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->canvas.context.name = name;
	fixture->canvas.context.uuid = uuid;
	fixture->canvas.context.signals = signals;
	fixture->canvas.flags = UINT32_C(0xa5a5a5a5);
	fixture->canvas.mix = mixed ? &fixture->mix : NULL;
	memcpy(&fixture->before, &fixture->canvas, sizeof(fixture->canvas));
	return fixture;
}

bool oracle_canvas_readonly_unchanged(const void *handle);
bool oracle_canvas_readonly_unchanged(const void *handle)
{
	const struct canvas_readonly_fixture *fixture = handle;
	return memcmp(&fixture->canvas, &fixture->before, sizeof(fixture->canvas)) == 0 && fixture->mix.video == NULL;
}

void oracle_canvas_readonly_free(void *handle);
void oracle_canvas_readonly_free(void *handle)
{
	free(handle);
}

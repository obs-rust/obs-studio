/* Original getter and field adapter, measured against the real private header. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_canvas_get_flags oracle_obs_canvas_get_flags
#include "obs-canvas-flags-getter.c.h"
#undef obs_canvas_get_flags
#define obs_canvas_rules_removed oracle_obs_canvas_rules_removed
#define obs_canvas_removed oracle_obs_canvas_removed
bool oracle_obs_canvas_removed(obs_canvas_t *canvas);
#include "obs-canvas-removed.c.h"
#undef obs_canvas_removed
#undef obs_canvas_rules_removed
#include "canvas_flags_fields.inc"

struct canvas_flags_fixture {
	struct obs_canvas canvas;
	struct obs_canvas before;
};

void oracle_canvas_flags_layout(size_t *layout);
void oracle_canvas_flags_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_canvas_flags_fields);
	layout[1] = _Alignof(struct obs_canvas_flags_fields);
	layout[2] = offsetof(struct obs_canvas_flags_fields, flags);
	layout[3] = sizeof(((struct obs_canvas *)0)->flags);
	layout[4] = _Alignof(uint32_t);
}

void *oracle_canvas_flags_new(uint32_t flags);
void *oracle_canvas_flags_new(uint32_t flags)
{
	struct canvas_flags_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->canvas.flags = flags;
	memcpy(&fixture->before, &fixture->canvas, sizeof(fixture->canvas));
	return fixture;
}

bool oracle_canvas_flags_unchanged(const void *handle);
bool oracle_canvas_flags_unchanged(const void *handle)
{
	const struct canvas_flags_fixture *fixture = handle;
	return memcmp(&fixture->canvas, &fixture->before, sizeof(fixture->canvas)) == 0;
}

void oracle_canvas_flags_free(void *handle);
void oracle_canvas_flags_free(void *handle)
{
	free(handle);
}

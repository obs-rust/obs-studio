/* Original queries and discovered production adapters, over real-header fixtures. */
#include <stddef.h>
#include <string.h>
#define obs core_texture_activity_oracle_core
#define obs_nv12_tex_active oracle_obs_nv12_tex_active
#define obs_p010_tex_active oracle_obs_p010_tex_active
#include "obs-core-texture-activity.c.h"
#undef obs_nv12_tex_active
#undef obs_p010_tex_active
#include "core_texture_activity_fields.inc"
#undef obs

static struct obs_core texture_core, texture_core_before;
static struct obs_canvas texture_canvas, texture_canvas_before;
static struct obs_core_video_mix texture_mix, texture_mix_before;
struct obs_core *core_texture_activity_oracle_core;

void oracle_core_texture_activity_set(unsigned depth, bool nv12, bool p010);
void oracle_core_texture_activity_set(unsigned depth, bool nv12, bool p010)
{
	memset(&texture_core, 0, sizeof(texture_core));
	memset(&texture_canvas, 0, sizeof(texture_canvas));
	memset(&texture_mix, 0, sizeof(texture_mix));
	core_texture_activity_oracle_core = depth ? &texture_core : NULL;
	texture_core.data.main_canvas = depth >= 2 ? &texture_canvas : NULL;
	texture_canvas.mix = depth >= 3 ? &texture_mix : NULL;
	texture_mix.using_nv12_tex = nv12;
	texture_mix.using_p010_tex = p010;
	memcpy(&texture_core_before, &texture_core, sizeof(texture_core));
	memcpy(&texture_canvas_before, &texture_canvas, sizeof(texture_canvas));
	memcpy(&texture_mix_before, &texture_mix, sizeof(texture_mix));
}

bool oracle_core_texture_activity_unchanged(void);
bool oracle_core_texture_activity_unchanged(void)
{
	return memcmp(&texture_core_before, &texture_core, sizeof(texture_core)) == 0 &&
	       memcmp(&texture_canvas_before, &texture_canvas, sizeof(texture_canvas)) == 0 &&
	       memcmp(&texture_mix_before, &texture_mix, sizeof(texture_mix)) == 0;
}

void oracle_core_texture_activity_layout(size_t *layout);
void oracle_core_texture_activity_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_core_texture_activity_fields);
	layout[1] = _Alignof(struct obs_core_texture_activity_fields);
	layout[2] = offsetof(struct obs_core_texture_activity_fields, nv12);
	layout[3] = offsetof(struct obs_core_texture_activity_fields, p010);
	layout[4] = sizeof(texture_mix.using_nv12_tex);
	layout[5] = _Alignof(bool);
}

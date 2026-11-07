/* Original C getter and discovered adapter against the real private header. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_canvas_get_video oracle_obs_canvas_get_video
#include "obs-canvas-video-getter.c.h"
#undef obs_canvas_get_video
#include "canvas_video_fields.inc"

struct canvas_video_fixture {
	struct obs_canvas canvas;
	struct obs_canvas before;
	struct obs_core_video_mix mix;
	struct obs_core_video_mix mix_before;
};

void oracle_canvas_video_layout(size_t *layout);
void oracle_canvas_video_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_canvas_video_fields);
	layout[1] = _Alignof(struct obs_canvas_video_fields);
	layout[2] = offsetof(struct obs_canvas_video_fields, mix);
	layout[3] = sizeof(((struct obs_core_video_mix *)0)->video);
	layout[4] = _Alignof(video_t *);
}

void *oracle_canvas_video_new(bool mixed, void *video);
void *oracle_canvas_video_new(bool mixed, void *video)
{
	struct canvas_video_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->canvas.mix = mixed ? &fixture->mix : NULL;
	fixture->mix.video = video;
	memcpy(&fixture->before, &fixture->canvas, sizeof(fixture->canvas));
	memcpy(&fixture->mix_before, &fixture->mix, sizeof(fixture->mix));
	return fixture;
}

bool oracle_canvas_video_unchanged(const void *handle);
bool oracle_canvas_video_unchanged(const void *handle)
{
	const struct canvas_video_fixture *fixture = handle;
	return memcmp(&fixture->canvas, &fixture->before, sizeof(fixture->canvas)) == 0 &&
	       memcmp(&fixture->mix, &fixture->mix_before, sizeof(fixture->mix)) == 0;
}

void oracle_canvas_video_free(void *handle);
void oracle_canvas_video_free(void *handle)
{
	free(handle);
}

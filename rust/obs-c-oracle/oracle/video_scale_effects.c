/* Original static C helpers, real private fields, and the production caller. */
#include <stdlib.h>
#include <string.h>
#include "libobs/obs-video-scale-effects.c.h"
#include "video_scale_effects_fields.inc"
#define VIDEO_SCALE_EFFECT_CALLER_EXTRACT
#include "test/cmocka/video-scale-effects-caller.c"

struct scale_effect_fixture {
	struct obs_core_video video;
	struct obs_core_video before;
	gs_effect_t identities[5];
};

void *oracle_video_scale_effect_new(uint32_t mask);
void *oracle_video_scale_effect_new(uint32_t mask)
{
	struct scale_effect_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	f->video.default_effect = (mask & 1) ? &f->identities[0] : NULL;
	f->video.bilinear_lowres_effect = (mask & 2) ? &f->identities[1] : NULL;
	f->video.bicubic_effect = (mask & 4) ? &f->identities[2] : NULL;
	f->video.lanczos_effect = (mask & 8) ? &f->identities[3] : NULL;
	f->video.area_effect = (mask & 16) ? &f->identities[4] : NULL;
	memcpy(&f->before, &f->video, sizeof(f->video));
	return f;
}
uint32_t oracle_video_scale_effect_loaded(const void *p);
uint32_t oracle_video_scale_effect_loaded(const void *p)
{
	const struct scale_effect_fixture *f = p;
	return loaded_scale_effects(&f->video);
}
void *oracle_video_scale_effect_selected(const void *p, int selector);
void *oracle_video_scale_effect_selected(const void *p, int selector)
{
	const struct scale_effect_fixture *f = p;
	return scale_effect_pointer(&f->video, (enum obs_video_scale_effect)selector);
}
bool oracle_video_scale_effect_unchanged(const void *p);
bool oracle_video_scale_effect_unchanged(const void *p)
{
	const struct scale_effect_fixture *f = p;
	return memcmp(&f->video, &f->before, sizeof(f->video)) == 0;
}
void oracle_video_scale_effect_free(void *p);
void oracle_video_scale_effect_free(void *p)
{
	free(p);
}

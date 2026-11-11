/* Compile the extracted original getter against the real private encoder layout. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_encoder_get_preferred_color_space oracle_obs_encoder_get_preferred_color_space
#include "obs-encoder-preferred-color-space.c.h"
#undef obs_encoder_get_preferred_color_space
#include "encoder_preferred_color_space_fields.inc"

void oracle_encoder_preferred_color_space_layout(size_t *layout);
void oracle_encoder_preferred_color_space_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_preferred_color_space_fields);
	layout[1] = _Alignof(struct obs_encoder_preferred_color_space_fields);
	layout[2] = offsetof(struct obs_encoder_preferred_color_space_fields, kind);
	layout[3] = offsetof(struct obs_encoder_preferred_color_space_fields, space);
	layout[4] = sizeof(enum obs_encoder_type);
	layout[5] = sizeof(enum video_colorspace);
}

struct preferred_color_space_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
};
void *oracle_encoder_preferred_color_space_new(int kind, uint32_t space);
void *oracle_encoder_preferred_color_space_new(int kind, uint32_t space)
{
	struct preferred_color_space_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.info.type = (enum obs_encoder_type)kind;
		fixture->encoder.orig_info.type = kind == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		fixture->encoder.preferred_space = (enum video_colorspace)space;
		fixture->encoder.preferred_format = VIDEO_FORMAT_NV12;
		fixture->encoder.preferred_range = VIDEO_RANGE_FULL;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_encoder_preferred_color_space_unchanged(void *encoder);
bool oracle_encoder_preferred_color_space_unchanged(void *encoder)
{
	struct preferred_color_space_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0;
}
void oracle_encoder_preferred_color_space_free(void *encoder);
void oracle_encoder_preferred_color_space_free(void *encoder)
{
	free(encoder);
}

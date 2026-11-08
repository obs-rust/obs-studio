/* Compile the extracted original getter against the real private encoder layout. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_encoder_get_preferred_video_format oracle_obs_encoder_get_preferred_video_format
#include "obs-encoder-preferred-format.c.h"
#undef obs_encoder_get_preferred_video_format
#include "encoder_preferred_format_fields.inc"

void oracle_encoder_preferred_format_layout(size_t *layout);
void oracle_encoder_preferred_format_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_preferred_format_fields);
	layout[1] = _Alignof(struct obs_encoder_preferred_format_fields);
	layout[2] = offsetof(struct obs_encoder_preferred_format_fields, kind);
	layout[3] = offsetof(struct obs_encoder_preferred_format_fields, format);
	layout[4] = sizeof(enum obs_encoder_type);
	layout[5] = sizeof(enum video_format);
}

struct preferred_format_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
};
void *oracle_encoder_preferred_format_new(int kind, uint32_t format);
void *oracle_encoder_preferred_format_new(int kind, uint32_t format)
{
	struct preferred_format_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.info.type = (enum obs_encoder_type)kind;
		fixture->encoder.orig_info.type = kind == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		fixture->encoder.preferred_format = (enum video_format)format;
		fixture->encoder.preferred_space = VIDEO_CS_709;
		fixture->encoder.preferred_range = VIDEO_RANGE_FULL;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_encoder_preferred_format_unchanged(void *encoder);
bool oracle_encoder_preferred_format_unchanged(void *encoder)
{
	struct preferred_format_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0;
}
void oracle_encoder_preferred_format_free(void *encoder);
void oracle_encoder_preferred_format_free(void *encoder)
{
	free(encoder);
}

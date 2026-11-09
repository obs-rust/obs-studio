/* Compile the extracted original getter against the real private encoder layout. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_encoder_get_preferred_range oracle_obs_encoder_get_preferred_range
#include "obs-encoder-preferred-range.c.h"
#undef obs_encoder_get_preferred_range
#include "encoder_preferred_range_fields.inc"

void oracle_encoder_preferred_range_layout(size_t *layout);
void oracle_encoder_preferred_range_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_preferred_range_fields);
	layout[1] = _Alignof(struct obs_encoder_preferred_range_fields);
	layout[2] = offsetof(struct obs_encoder_preferred_range_fields, kind);
	layout[3] = offsetof(struct obs_encoder_preferred_range_fields, range);
	layout[4] = sizeof(enum obs_encoder_type);
	layout[5] = sizeof(enum video_range_type);
}

struct preferred_range_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
};
void *oracle_encoder_preferred_range_new(int kind, uint32_t range);
void *oracle_encoder_preferred_range_new(int kind, uint32_t range)
{
	struct preferred_range_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.info.type = (enum obs_encoder_type)kind;
		fixture->encoder.orig_info.type = kind == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		fixture->encoder.preferred_range = (enum video_range_type)range;
		fixture->encoder.preferred_space = VIDEO_CS_709;
		fixture->encoder.preferred_format = VIDEO_FORMAT_NV12;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_encoder_preferred_range_unchanged(void *encoder);
bool oracle_encoder_preferred_range_unchanged(void *encoder)
{
	struct preferred_range_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0;
}
void oracle_encoder_preferred_range_free(void *encoder);
void oracle_encoder_preferred_range_free(void *encoder)
{
	free(encoder);
}

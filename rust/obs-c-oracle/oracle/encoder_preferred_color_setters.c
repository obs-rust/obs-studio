/* Original extracted C setters and real private-layout adapter. No exclusions. */
#include <stddef.h>
#include <string.h>
#define obs_encoder_set_preferred_color_space oracle_obs_encoder_set_preferred_color_space
#define obs_encoder_set_preferred_range oracle_obs_encoder_set_preferred_range
#include "obs-encoder-preferred-color-setters.c.h"
#undef obs_encoder_set_preferred_range
#undef obs_encoder_set_preferred_color_space
#include "encoder_preferred_color_setters_fields.inc"

extern void obs_encoder_set_preferred_color_space(obs_encoder_t *encoder, enum video_colorspace space);
extern void obs_encoder_set_preferred_range(obs_encoder_t *encoder, enum video_range_type range);

void oracle_encoder_preferred_color_setters_layout(size_t *layout);
void oracle_encoder_preferred_color_setters_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_preferred_color_setters_fields);
	layout[1] = _Alignof(struct obs_encoder_preferred_color_setters_fields);
	layout[2] = offsetof(struct obs_encoder_preferred_color_setters_fields, kind);
	layout[3] = offsetof(struct obs_encoder_preferred_color_setters_fields, space);
	layout[4] = offsetof(struct obs_encoder_preferred_color_setters_fields, range);
	layout[5] = sizeof(enum obs_encoder_type);
	layout[6] = _Alignof(enum obs_encoder_type);
	layout[7] = sizeof(enum video_colorspace);
	layout[8] = _Alignof(enum video_colorspace);
	layout[9] = sizeof(enum video_range_type);
	layout[10] = _Alignof(enum video_range_type);
}

bool oracle_encoder_preferred_color_setters_check(int kind, uint32_t space, uint32_t range, const uint32_t *values,
						  const bool *range_writes, size_t count);
bool oracle_encoder_preferred_color_setters_check(int kind, uint32_t space, uint32_t range, const uint32_t *values,
						  const bool *range_writes, size_t count)
{
	struct obs_encoder original = {0}, replacement, expected;
	original.info.type = (enum obs_encoder_type)kind;
	original.orig_info.type = kind == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
	original.preferred_format = VIDEO_FORMAT_NV12;
	original.preferred_space = (enum video_colorspace)space;
	original.preferred_range = (enum video_range_type)range;
	memcpy(&replacement, &original, sizeof(original));
	memcpy(&expected, &original, sizeof(original));
	struct obs_encoder_preferred_color_setters_fields fields =
		obs_encoder_preferred_color_setters_fields(&replacement);
	if (fields.kind != &replacement.info.type || fields.space != &replacement.preferred_space ||
	    fields.range != &replacement.preferred_range)
		return false;
	for (size_t i = 0; i < count; i++) {
		if (kind == OBS_ENCODER_VIDEO) {
			if (range_writes[i])
				expected.preferred_range = (enum video_range_type)values[i];
			else
				expected.preferred_space = (enum video_colorspace)values[i];
		}
		for (size_t repeat = 0; repeat < 2; repeat++) {
			if (range_writes[i]) {
				oracle_obs_encoder_set_preferred_range(&original, (enum video_range_type)values[i]);
				obs_encoder_set_preferred_range(&replacement, (enum video_range_type)values[i]);
			} else {
				oracle_obs_encoder_set_preferred_color_space(&original,
									     (enum video_colorspace)values[i]);
				obs_encoder_set_preferred_color_space(&replacement, (enum video_colorspace)values[i]);
			}
			if (memcmp(&original, &replacement, sizeof(original)) != 0 ||
			    memcmp(&replacement, &expected, sizeof(expected)) != 0)
				return false;
		}
	}
	return true;
}

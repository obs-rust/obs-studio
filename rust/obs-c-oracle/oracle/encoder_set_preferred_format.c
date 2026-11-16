/* Original extracted C setter and real private-layout adapter. No exclusions. */
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_set_preferred_video_format oracle_obs_encoder_set_preferred_video_format
#include "obs-encoder-set-preferred-format.c.h"
#undef obs_encoder_set_preferred_video_format
#include "encoder_set_preferred_format_fields.inc"

extern void obs_encoder_set_preferred_video_format(obs_encoder_t *encoder, enum video_format format);

void oracle_encoder_set_preferred_format_layout(size_t *layout);
void oracle_encoder_set_preferred_format_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_set_preferred_format_fields);
	layout[1] = _Alignof(struct obs_encoder_set_preferred_format_fields);
	layout[2] = offsetof(struct obs_encoder_set_preferred_format_fields, kind);
	layout[3] = offsetof(struct obs_encoder_set_preferred_format_fields, format);
	layout[4] = sizeof(enum obs_encoder_type);
	layout[5] = _Alignof(enum obs_encoder_type);
	layout[6] = sizeof(enum video_format);
	layout[7] = _Alignof(enum video_format);
}

bool oracle_encoder_set_preferred_format_check(int kind, uint32_t before, uint32_t format);
bool oracle_encoder_set_preferred_format_check(int kind, uint32_t before, uint32_t format)
{
	struct obs_encoder original = {0}, replacement, expected;
	original.info.type = (enum obs_encoder_type)kind;
	original.orig_info.type = kind == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
	original.preferred_format = (enum video_format)before;
	original.preferred_space = VIDEO_CS_709;
	original.preferred_range = VIDEO_RANGE_FULL;
	memcpy(&replacement, &original, sizeof(original));
	memcpy(&expected, &original, sizeof(original));
	if (kind == OBS_ENCODER_VIDEO)
		expected.preferred_format = (enum video_format)format;
	oracle_obs_encoder_set_preferred_video_format(&original, (enum video_format)format);
	obs_encoder_set_preferred_video_format(&replacement, (enum video_format)format);
	return memcmp(&original, &replacement, sizeof(original)) == 0 &&
	       memcmp(&replacement, &expected, sizeof(expected)) == 0;
}

#if defined(__GNUC__) || defined(__clang__)
static void set_format_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void set_format_capture(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(*(unsigned *)param)++;
}
bool oracle_encoder_set_preferred_format_null_silent(bool rust);
bool oracle_encoder_set_preferred_format_null_silent(bool rust)
{
	unsigned logs = 0;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(set_format_capture, &logs);
	if (rust)
		obs_encoder_set_preferred_video_format(NULL, VIDEO_FORMAT_NV12);
	else
		oracle_obs_encoder_set_preferred_video_format(NULL, VIDEO_FORMAT_NV12);
	base_set_log_handler(old_handler, old_param);
	return logs == 0;
}

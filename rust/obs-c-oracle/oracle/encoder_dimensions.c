/* Original C, oracle helper, discovered adapter, and explicit media callbacks. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_get_width oracle_obs_encoder_get_width
#define obs_encoder_get_height oracle_obs_encoder_get_height
#define obs_encoder_get_name oracle_encoder_dimensions_name
#define obs_encoder_size_video_route oracle_obs_encoder_size_video_route
#define video_output_get_width oracle_encoder_dimensions_video_width
#define video_output_get_height oracle_encoder_dimensions_video_height
#include "obs-encoder-dimensions.c.h"
#undef video_output_get_height
#undef video_output_get_width
#undef obs_encoder_size_video_route
#undef obs_encoder_get_name
#undef obs_encoder_get_height
#undef obs_encoder_get_width
#include "encoder_dimensions_fields.inc"

const char *oracle_encoder_dimensions_name(const obs_encoder_t *encoder)
{
	return encoder->context.name;
}
typedef uint32_t (*dimensions_media_getter)(const video_t *);
static dimensions_media_getter dimensions_width;
static dimensions_media_getter dimensions_height;
uint32_t oracle_encoder_dimensions_video_width(const video_t *video)
{
	return dimensions_width(video);
}
uint32_t oracle_encoder_dimensions_video_height(const video_t *video)
{
	return dimensions_height(video);
}
void oracle_encoder_dimensions_layout(size_t *layout);
void oracle_encoder_dimensions_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_dimensions_fields);
	layout[1] = _Alignof(struct obs_encoder_dimensions_fields);
	layout[2] = offsetof(struct obs_encoder_dimensions_fields, type);
	layout[3] = offsetof(struct obs_encoder_dimensions_fields, media);
	layout[4] = offsetof(struct obs_encoder_dimensions_fields, width);
	layout[5] = offsetof(struct obs_encoder_dimensions_fields, height);
	layout[6] = offsetof(struct obs_encoder_dimensions_fields, name);
	layout[7] = sizeof(enum obs_encoder_type);
	layout[8] = _Alignof(enum obs_encoder_type);
	layout[9] = OBS_ENCODER_AUDIO;
	layout[10] = OBS_ENCODER_VIDEO;
}
struct dimensions_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
	char name[64];
};
void *oracle_encoder_dimensions_new(int type, void *media, uint32_t width, uint32_t height);
void *oracle_encoder_dimensions_new(int type, void *media, uint32_t width, uint32_t height)
{
	struct dimensions_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.info.type = (enum obs_encoder_type)type;
		fixture->encoder.orig_info.type = type == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		snprintf(fixture->name, sizeof(fixture->name), "%s", "dimensions-%s");
		fixture->encoder.context.name = fixture->name;
		fixture->encoder.media = media;
		fixture->encoder.scaled_width = width;
		fixture->encoder.scaled_height = height;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_encoder_dimensions_unchanged(void *encoder);
bool oracle_encoder_dimensions_unchanged(void *encoder)
{
	struct dimensions_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0 &&
	       strcmp(fixture->name, "dimensions-%s") == 0;
}
void oracle_encoder_dimensions_free(void *encoder);
void oracle_encoder_dimensions_free(void *encoder)
{
	free(encoder);
}
struct dimensions_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void dimensions_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void dimensions_capture(int level, const char *format, va_list args, void *param)
{
	struct dimensions_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
typedef uint32_t (*dimensions_getter)(const obs_encoder_t *);
bool oracle_encoder_dimensions_probe(const obs_encoder_t *encoder, dimensions_getter width, dimensions_getter height,
				     dimensions_media_getter media_width, dimensions_media_getter media_height);
bool oracle_encoder_dimensions_probe(const obs_encoder_t *encoder, dimensions_getter width, dimensions_getter height,
				     dimensions_media_getter media_width, dimensions_media_getter media_height)
{
	dimensions_width = media_width;
	dimensions_height = media_height;
	dimensions_getter getters[] = {width, height};
	dimensions_getter originals[] = {oracle_obs_encoder_get_width, oracle_obs_encoder_get_height};
	log_handler_t old_handler;
	void *old_param;
	bool equal = true;
	base_get_log_handler(&old_handler, &old_param);
	for (size_t i = 0; i < 2; i++) {
		struct dimensions_log rust_log = {0}, c_log = {0};
		base_set_log_handler(dimensions_capture, &rust_log);
		uint32_t rust_value = getters[i](encoder);
		base_set_log_handler(dimensions_capture, &c_log);
		uint32_t c_value = originals[i](encoder);
		equal = equal && rust_value == c_value && rust_log.count == c_log.count &&
			rust_log.level == c_log.level && strcmp(rust_log.message, c_log.message) == 0;
	}
	base_set_log_handler(old_handler, old_param);
	dimensions_width = NULL;
	dimensions_height = NULL;
	return equal;
}

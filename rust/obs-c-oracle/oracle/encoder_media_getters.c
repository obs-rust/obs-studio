/* Unmodified original getters and the real production field adapter. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_video oracle_obs_encoder_video
#define obs_encoder_parent_video oracle_obs_encoder_parent_video
#define obs_encoder_audio oracle_obs_encoder_audio
#define obs_encoder_get_name oracle_encoder_media_name
#define obs_encoder_audio_start_not_video oracle_obs_encoder_audio_start_not_video
#define obs_encoder_bind_not_audio oracle_obs_encoder_bind_not_audio
#include "obs-encoder-media-getters.c.h"
#undef obs_encoder_bind_not_audio
#undef obs_encoder_audio_start_not_video
#undef obs_encoder_get_name
#undef obs_encoder_audio
#undef obs_encoder_parent_video
#undef obs_encoder_video
#include "encoder_media_getters_fields.inc"

const char *oracle_encoder_media_name(const obs_encoder_t *encoder)
{
	return encoder->context.name;
}

struct encoder_media_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
	char name[64];
};

void *oracle_encoder_media_new(int type, void *media, void *override, bool active, bool initialized, const char *name);
void *oracle_encoder_media_new(int type, void *media, void *override, bool active, bool initialized, const char *name)
{
	struct encoder_media_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.info.type = (enum obs_encoder_type)type;
		fixture->encoder.orig_info.type = type == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		fixture->encoder.media = media;
		fixture->encoder.fps_override = override;
		fixture->encoder.active = active;
		fixture->encoder.initialized = initialized;
		snprintf(fixture->name, sizeof(fixture->name), "%s", name);
		fixture->encoder.context.name = fixture->name;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->encoder));
	}
	return fixture;
}

bool oracle_encoder_media_unchanged(const void *handle);
bool oracle_encoder_media_unchanged(const void *handle)
{
	const struct encoder_media_fixture *fixture = handle;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0;
}

void oracle_encoder_media_free(void *handle);
void oracle_encoder_media_free(void *handle)
{
	/* Media tokens are borrowed; this fixture owns only its own allocation. */
	free(handle);
}

void oracle_encoder_media_layout(size_t *layout);
void oracle_encoder_media_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_media_getters_fields);
	layout[1] = _Alignof(struct obs_encoder_media_getters_fields);
	layout[2] = offsetof(struct obs_encoder_media_getters_fields, type);
	layout[3] = offsetof(struct obs_encoder_media_getters_fields, media);
	layout[4] = offsetof(struct obs_encoder_media_getters_fields, fps_override);
	layout[5] = offsetof(struct obs_encoder_media_getters_fields, name);
	layout[6] = sizeof(int);
	layout[7] = _Alignof(int);
	layout[8] = OBS_ENCODER_AUDIO;
	layout[9] = OBS_ENCODER_VIDEO;
}

struct encoder_media_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void encoder_media_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void encoder_media_capture(int level, const char *format, va_list args, void *param)
{
	struct encoder_media_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void *(*encoder_media_getter)(const obs_encoder_t *);
bool oracle_encoder_media_probe(const obs_encoder_t *encoder, encoder_media_getter video, encoder_media_getter parent,
				encoder_media_getter audio);
bool oracle_encoder_media_probe(const obs_encoder_t *encoder, encoder_media_getter video, encoder_media_getter parent,
				encoder_media_getter audio)
{
	log_handler_t saved_handler;
	void *saved_param;
	base_get_log_handler(&saved_handler, &saved_param);
	bool equal = true;
	encoder_media_getter rust[] = {video, parent, audio};
	for (size_t i = 0; i < 3; i++) {
		struct encoder_media_log r = {0}, c = {0};
		base_set_log_handler(encoder_media_capture, &r);
		void *actual = rust[i](encoder);
		base_set_log_handler(encoder_media_capture, &c);
		void *expected = i == 0   ? (void *)oracle_obs_encoder_video(encoder)
				 : i == 1 ? (void *)oracle_obs_encoder_parent_video(encoder)
					  : (void *)oracle_obs_encoder_audio(encoder);
		equal = equal && actual == expected && r.count == c.count && r.level == c.level &&
			strcmp(r.message, c.message) == 0;
	}
	base_set_log_handler(saved_handler, saved_param);
	return equal;
}

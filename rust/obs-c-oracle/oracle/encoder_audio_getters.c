/* Original C with oracle helper routing and explicit production callbacks. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_get_sample_rate oracle_obs_encoder_get_sample_rate
#define obs_encoder_get_frame_size oracle_obs_encoder_get_frame_size
#define obs_encoder_get_mixer_index oracle_obs_encoder_get_mixer_index
#define obs_encoder_get_name oracle_encoder_audio_name
#define obs_encoder_size_rate_route oracle_obs_encoder_size_rate_route
#define audio_output_get_sample_rate oracle_encoder_audio_media_rate
#include "obs-encoder-audio-getters.c.h"
#undef audio_output_get_sample_rate
#undef obs_encoder_size_rate_route
#undef obs_encoder_get_name
#undef obs_encoder_get_mixer_index
#undef obs_encoder_get_frame_size
#undef obs_encoder_get_sample_rate
#include "encoder_audio_getters_fields.inc"

const char *oracle_encoder_audio_name(const obs_encoder_t *encoder)
{
	return encoder->context.name;
}
typedef uint32_t (*audio_rate_getter)(const audio_t *);
static audio_rate_getter audio_rate;
uint32_t oracle_encoder_audio_media_rate(const audio_t *audio)
{
	return audio_rate(audio);
}
void oracle_encoder_audio_layout(size_t *layout);
void oracle_encoder_audio_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_audio_getters_fields);
	layout[1] = _Alignof(struct obs_encoder_audio_getters_fields);
	layout[2] = offsetof(struct obs_encoder_audio_getters_fields, type);
	layout[3] = offsetof(struct obs_encoder_audio_getters_fields, media);
	layout[4] = offsetof(struct obs_encoder_audio_getters_fields, rate);
	layout[5] = offsetof(struct obs_encoder_audio_getters_fields, frame_size);
	layout[6] = offsetof(struct obs_encoder_audio_getters_fields, mixer_index);
	layout[7] = offsetof(struct obs_encoder_audio_getters_fields, name);
	layout[8] = sizeof(enum obs_encoder_type);
	layout[9] = _Alignof(enum obs_encoder_type);
	layout[10] = OBS_ENCODER_AUDIO;
	layout[11] = OBS_ENCODER_VIDEO;
}
struct audio_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
	char name[64];
};
void *oracle_encoder_audio_new(int type, void *media, uint32_t rate, size_t framesize, size_t mixer);
void *oracle_encoder_audio_new(int type, void *media, uint32_t rate, size_t framesize, size_t mixer)
{
	struct audio_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.info.type = (enum obs_encoder_type)type;
		fixture->encoder.orig_info.type = type == OBS_ENCODER_AUDIO ? OBS_ENCODER_VIDEO : OBS_ENCODER_AUDIO;
		snprintf(fixture->name, sizeof(fixture->name), "%s", "audio-%s");
		fixture->encoder.context.name = fixture->name;
		fixture->encoder.media = media;
		fixture->encoder.samplerate = rate;
		fixture->encoder.framesize = framesize;
		fixture->encoder.mixer_idx = mixer;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_encoder_audio_unchanged(void *encoder);
bool oracle_encoder_audio_unchanged(void *encoder)
{
	struct audio_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0 &&
	       strcmp(fixture->name, "audio-%s") == 0;
}
void oracle_encoder_audio_free(void *encoder);
void oracle_encoder_audio_free(void *encoder)
{
	free(encoder);
}
struct audio_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void audio_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void audio_capture(int level, const char *format, va_list args, void *param)
{
	struct audio_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
typedef size_t (*audio_size_getter)(const obs_encoder_t *);
typedef uint32_t (*audio_public_rate_getter)(const obs_encoder_t *);
bool oracle_encoder_audio_probe(const obs_encoder_t *encoder, audio_public_rate_getter rate,
				audio_size_getter frame_size, audio_size_getter mixer_index,
				audio_rate_getter media_rate);
bool oracle_encoder_audio_probe(const obs_encoder_t *encoder, audio_public_rate_getter rate,
				audio_size_getter frame_size, audio_size_getter mixer_index,
				audio_rate_getter media_rate)
{
	audio_rate = media_rate;
	log_handler_t old_handler;
	void *old_param;
	bool equal = true;
	base_get_log_handler(&old_handler, &old_param);
	struct audio_log rust_log = {0}, c_log = {0};
	base_set_log_handler(audio_capture, &rust_log);
	uint32_t rust_rate = rate(encoder);
	base_set_log_handler(audio_capture, &c_log);
	uint32_t c_rate = oracle_obs_encoder_get_sample_rate(encoder);
	equal = rust_rate == c_rate && rust_log.count == c_log.count && rust_log.level == c_log.level &&
		strcmp(rust_log.message, c_log.message) == 0;
	audio_size_getter getters[] = {frame_size, mixer_index};
	audio_size_getter originals[] = {oracle_obs_encoder_get_frame_size, oracle_obs_encoder_get_mixer_index};
	for (size_t i = 0; i < 2; i++) {
		memset(&rust_log, 0, sizeof(rust_log));
		memset(&c_log, 0, sizeof(c_log));
		base_set_log_handler(audio_capture, &rust_log);
		size_t rust_value = getters[i](encoder);
		base_set_log_handler(audio_capture, &c_log);
		size_t c_value = originals[i](encoder);
		equal = equal && rust_value == c_value && rust_log.count == c_log.count &&
			rust_log.level == c_log.level && strcmp(rust_log.message, c_log.message) == 0;
	}
	base_set_log_handler(old_handler, old_param);
	audio_rate = NULL;
	return equal;
}

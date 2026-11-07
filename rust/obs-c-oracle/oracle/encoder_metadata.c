/* Original extracted getters and production field adapter over real headers. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_get_codec oracle_obs_encoder_get_codec
#define obs_encoder_get_type oracle_obs_encoder_get_type
#define obs_encoder_get_id oracle_obs_encoder_get_id
#define obs_encoder_get_type_data oracle_obs_encoder_get_type_data
#define obs_encoder_get_caps oracle_obs_encoder_get_caps
#include "obs-encoder-metadata-getters.c.h"
#undef obs_encoder_get_codec
#undef obs_encoder_get_type
#undef obs_encoder_get_id
#undef obs_encoder_get_type_data
#undef obs_encoder_get_caps
#include "encoder_metadata_fields.inc"

/* The owning parity harness supplies either C or Rust getters. Unrelated
 * oracle consumers must not acquire a link dependency on obs-output. */
typedef const char *(*metadata_string_getter)(const obs_encoder_t *encoder);
typedef enum obs_encoder_type (*metadata_type_getter)(const obs_encoder_t *encoder);
typedef void *(*metadata_data_getter)(obs_encoder_t *encoder);
typedef uint32_t (*metadata_caps_getter)(const obs_encoder_t *encoder);

void oracle_encoder_metadata_layout(size_t *layout);
void oracle_encoder_metadata_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_metadata_fields);
	layout[1] = _Alignof(struct obs_encoder_metadata_fields);
	layout[2] = offsetof(struct obs_encoder_metadata_fields, codec);
	layout[3] = offsetof(struct obs_encoder_metadata_fields, type);
	layout[4] = offsetof(struct obs_encoder_metadata_fields, id);
	layout[5] = offsetof(struct obs_encoder_metadata_fields, type_data);
	layout[6] = offsetof(struct obs_encoder_metadata_fields, caps);
	layout[7] = sizeof(enum obs_encoder_type);
	layout[8] = _Alignof(enum obs_encoder_type);
	layout[9] = OBS_ENCODER_AUDIO;
	layout[10] = OBS_ENCODER_VIDEO;
}

struct metadata_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
};

void *oracle_encoder_metadata_new(const char *codec, int type, const char *id, void *data, uint32_t caps);
void *oracle_encoder_metadata_new(const char *codec, int type, const char *id, void *data, uint32_t caps)
{
	struct metadata_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.info.codec = codec;
		fixture->encoder.info.type = (enum obs_encoder_type)type;
		fixture->encoder.info.id = "rerouted";
		fixture->encoder.info.type_data = &fixture->encoder.encoded_frames;
		fixture->encoder.info.caps = ~caps;
		fixture->encoder.orig_info.codec = "original-codec";
		fixture->encoder.orig_info.type = type == OBS_ENCODER_AUDIO ? OBS_ENCODER_VIDEO : OBS_ENCODER_AUDIO;
		fixture->encoder.orig_info.id = id;
		fixture->encoder.orig_info.type_data = data;
		fixture->encoder.orig_info.caps = caps;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}

bool oracle_encoder_metadata_unchanged(void *encoder);
bool oracle_encoder_metadata_unchanged(void *encoder)
{
	struct metadata_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0;
}

void oracle_encoder_metadata_free(void *encoder);
void oracle_encoder_metadata_free(void *encoder)
{
	free(encoder);
}

struct metadata_log {
	unsigned count;
	int level;
	char message[128];
};

/* va_list printf callback, matching the format-checked log-capture oracles. */
#if defined(__GNUC__) || defined(__clang__)
static void metadata_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void metadata_capture(int level, const char *format, va_list args, void *param)
{
	struct metadata_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static bool metadata_check_log(struct metadata_log *log, const char *function)
{
	char expected[128];
	snprintf(expected, sizeof(expected), "%s: Null 'encoder' parameter", function);
	bool ok = log->count == 1 && log->level == LOG_DEBUG && strcmp(log->message, expected) == 0;
	log->count = 0;
	return ok;
}

bool oracle_encoder_metadata_null_log(metadata_string_getter codec, metadata_type_getter type,
				      metadata_string_getter id, metadata_data_getter type_data,
				      metadata_caps_getter caps);
bool oracle_encoder_metadata_null_log(metadata_string_getter codec, metadata_type_getter type,
				      metadata_string_getter id, metadata_data_getter type_data,
				      metadata_caps_getter caps)
{
	struct metadata_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(metadata_capture, &log);
	bool ok = codec(NULL) == NULL;
	ok &= metadata_check_log(&log, "obs_encoder_get_codec");
	ok &= type(NULL) == OBS_ENCODER_AUDIO;
	ok &= metadata_check_log(&log, "obs_encoder_get_type");
	ok &= id(NULL) == NULL;
	ok &= metadata_check_log(&log, "obs_encoder_get_id");
	ok &= type_data(NULL) == NULL;
	ok &= metadata_check_log(&log, "obs_encoder_get_type_data");
	ok &= caps(NULL) == 0;
	ok &= metadata_check_log(&log, "obs_encoder_get_caps");
	base_set_log_handler(old_handler, old_param);
	return ok;
}

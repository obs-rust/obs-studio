/* Original getter and production accessor over the real private encoder layout. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_get_settings oracle_obs_encoder_get_settings
#include "obs-encoder-settings.c.h"
#undef obs_encoder_get_settings
#include "encoder_settings_fields.inc"

struct encoder_settings_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
};

void *oracle_encoder_settings_new(obs_data_t *settings, uint32_t flags);
void *oracle_encoder_settings_new(obs_data_t *settings, uint32_t flags)
{
	struct encoder_settings_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.context.settings = settings;
		fixture->encoder.info.caps = flags;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->encoder));
	}
	return fixture;
}

bool oracle_encoder_settings_unchanged(const void *handle);
bool oracle_encoder_settings_unchanged(const void *handle)
{
	const struct encoder_settings_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->encoder, sizeof(fixture->encoder)) == 0;
}

void oracle_encoder_settings_free(void *handle);
void oracle_encoder_settings_free(void *handle)
{
	/* The Rust test owns the settings object and every returned reference. */
	free(handle);
}

static void *encoder_settings_original_getter(const obs_encoder_t *encoder)
{
	return oracle_obs_encoder_get_settings(encoder);
}

struct encoder_settings_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void encoder_settings_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void encoder_settings_capture(int level, const char *format, va_list args, void *param)
{
	struct encoder_settings_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void *(*encoder_settings_getter)(const obs_encoder_t *);
typedef void (*encoder_settings_get_log)(log_handler_t *, void **);
typedef void (*encoder_settings_set_log)(log_handler_t, void *);
bool oracle_encoder_settings_logs(encoder_settings_getter proc, encoder_settings_get_log get_log,
				  encoder_settings_set_log set_log);
bool oracle_encoder_settings_logs(encoder_settings_getter proc, encoder_settings_get_log get_log,
				  encoder_settings_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	encoder_settings_getter rust[] = {proc};
	encoder_settings_getter original[] = {encoder_settings_original_getter};
	const char *expected[] = {"obs_encoder_get_settings: Null 'encoder' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 1; i++) {
		struct encoder_settings_log r = {0}, c = {0};
		set_log(encoder_settings_capture, &r);
		void *rust_value = rust[i](NULL);
		set_log(encoder_settings_capture, &c);
		void *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}

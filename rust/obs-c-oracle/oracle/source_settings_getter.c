/* Original getter and production accessor over the real private source layout. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_settings oracle_obs_source_get_settings
#include "obs-source-settings-getter.c.h"
#undef obs_source_get_settings
#include "source_settings_getter_fields.inc"

struct source_settings_fixture {
	struct obs_source source;
	struct obs_source before;
};

void *oracle_source_settings_new(obs_data_t *settings, uint32_t flags);
void *oracle_source_settings_new(obs_data_t *settings, uint32_t flags)
{
	struct source_settings_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.context.settings = settings;
		fixture->source.info.output_flags = flags;
		memcpy(&fixture->before, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

bool oracle_source_settings_unchanged(const void *handle);
bool oracle_source_settings_unchanged(const void *handle)
{
	const struct source_settings_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->source, sizeof(fixture->source)) == 0;
}

void oracle_source_settings_free(void *handle);
void oracle_source_settings_free(void *handle)
{
	/* The Rust test owns the settings object and every returned reference. */
	free(handle);
}

static void *source_settings_original_getter(const obs_source_t *source)
{
	return oracle_obs_source_get_settings(source);
}

struct source_settings_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_settings_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_settings_capture(int level, const char *format, va_list args, void *param)
{
	struct source_settings_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void *(*source_settings_getter)(const obs_source_t *);
typedef void (*source_settings_get_log)(log_handler_t *, void **);
typedef void (*source_settings_set_log)(log_handler_t, void *);
bool oracle_source_settings_logs(source_settings_getter proc, source_settings_get_log get_log,
				 source_settings_set_log set_log);
bool oracle_source_settings_logs(source_settings_getter proc, source_settings_get_log get_log,
				 source_settings_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	source_settings_getter rust[] = {proc};
	source_settings_getter original[] = {source_settings_original_getter};
	const char *expected[] = {"obs_source_get_settings: Null 'source' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 1; i++) {
		struct source_settings_log r = {0}, c = {0};
		set_log(source_settings_capture, &r);
		void *rust_value = rust[i](NULL);
		set_log(source_settings_capture, &c);
		void *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}

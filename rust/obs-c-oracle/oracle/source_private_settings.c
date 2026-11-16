/* Original private-settings getter and production accessor over the real private source layout. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_private_settings oracle_obs_source_get_private_settings
#include "obs-source-private-settings.c.h"
#undef obs_source_get_private_settings
#include "source_private_settings_fields.inc"

struct source_private_settings_fixture {
	struct obs_source source;
	struct obs_source before;
};

void *oracle_source_private_settings_new(obs_data_t *private_settings, obs_data_t *settings, uint32_t flags);
void *oracle_source_private_settings_new(obs_data_t *private_settings, obs_data_t *settings, uint32_t flags)
{
	struct source_private_settings_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.private_settings = private_settings;
		fixture->source.context.settings = settings;
		fixture->source.info.output_flags = flags;
		memcpy(&fixture->before, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

bool oracle_source_private_settings_unchanged(const void *handle);
bool oracle_source_private_settings_unchanged(const void *handle)
{
	const struct source_private_settings_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->source, sizeof(fixture->source)) == 0;
}

void *oracle_source_private_settings_read(const void *handle);
void *oracle_source_private_settings_read(const void *handle)
{
	const struct source_private_settings_fixture *fixture = handle;
	return obs_source_private_settings_fields(&fixture->source);
}

void oracle_source_private_settings_free(void *handle);
void oracle_source_private_settings_free(void *handle)
{
	/* The Rust test owns both data objects and every returned reference. */
	free(handle);
}

static void *source_private_settings_original_getter(obs_source_t *source)
{
	return oracle_obs_source_get_private_settings(source);
}

struct source_private_settings_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_private_settings_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_private_settings_capture(int level, const char *format, va_list args, void *param)
{
	struct source_private_settings_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void *(*source_private_settings_getter)(obs_source_t *);
typedef void (*source_private_settings_get_log)(log_handler_t *, void **);
typedef void (*source_private_settings_set_log)(log_handler_t, void *);
bool oracle_source_private_settings_logs(source_private_settings_getter proc, source_private_settings_get_log get_log,
					 source_private_settings_set_log set_log);
bool oracle_source_private_settings_logs(source_private_settings_getter proc, source_private_settings_get_log get_log,
					 source_private_settings_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct source_private_settings_log r = {0}, c = {0};
	set_log(source_private_settings_capture, &r);
	void *rust_value = proc(NULL);
	set_log(source_private_settings_capture, &c);
	void *c_value = source_private_settings_original_getter(NULL);
	set_log(old_handler, old_param);
	return !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
	       c.level == LOG_DEBUG &&
	       strcmp(r.message, "obs_source_get_private_settings: Null 'source' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}

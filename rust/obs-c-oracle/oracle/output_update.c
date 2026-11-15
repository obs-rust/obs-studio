/* Original C update and the production field adapter over the real private
 * output layout, with a recording stand-in for the plugin's update callback.
 * Both sides merge through the same obs_data_apply(). */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_update oracle_obs_output_update
#include "obs-output-update.c.h"
#undef obs_output_update
#include "output_update_fields.inc"

struct output_update_fixture {
	struct obs_output output;
	int plugin_data;
	unsigned calls;
	bool data_ok;
	bool settings_ok;
	char *seen;
};

static void output_update_record(void *data, obs_data_t *settings)
{
	struct output_update_fixture *fixture =
		(struct output_update_fixture *)((char *)data - offsetof(struct output_update_fixture, plugin_data));
	fixture->calls++;
	fixture->data_ok = data == &fixture->plugin_data;
	fixture->settings_ok = settings == fixture->output.context.settings;
	free(fixture->seen);
	const char *json = settings ? obs_data_get_json(settings) : NULL;
	fixture->seen = json ? strdup(json) : NULL;
}

void *oracle_output_update_new(obs_data_t *settings, bool has_update, uint32_t flags);
void *oracle_output_update_new(obs_data_t *settings, bool has_update, uint32_t flags)
{
	struct output_update_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.context.settings = settings;
		fixture->output.context.data = &fixture->plugin_data;
		fixture->output.info.update = has_update ? output_update_record : NULL;
		fixture->output.info.flags = flags;
	}
	return fixture;
}

obs_output_t *oracle_output_update_output(void *handle);
obs_output_t *oracle_output_update_output(void *handle)
{
	return &((struct output_update_fixture *)handle)->output;
}

/* The callback's record: the number of calls, whether the last one saw the
 * fixture's own plugin data and stored settings, and the settings' JSON
 * then (NULL if it never ran). */
unsigned oracle_output_update_calls(const void *handle, bool *identity, const char **seen);
unsigned oracle_output_update_calls(const void *handle, bool *identity, const char **seen)
{
	const struct output_update_fixture *fixture = handle;
	*identity = fixture->data_ok && fixture->settings_ok;
	*seen = fixture->seen;
	return fixture->calls;
}

/* The output still holds its settings object, its data and its callback. */
bool oracle_output_update_unchanged(const void *handle, obs_data_t *settings, bool has_update);
bool oracle_output_update_unchanged(const void *handle, obs_data_t *settings, bool has_update)
{
	const struct output_update_fixture *fixture = handle;
	return fixture->output.context.settings == settings && fixture->output.context.data == &fixture->plugin_data &&
	       (fixture->output.info.update != NULL) == has_update;
}

void oracle_output_update_free(void *handle);
void oracle_output_update_free(void *handle)
{
	struct output_update_fixture *fixture = handle;
	if (fixture)
		free(fixture->seen);
	free(fixture);
}

struct output_update_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_update_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_update_capture(int level, const char *format, va_list args, void *param)
{
	struct output_update_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Symbol renaming changes __func__ only in the oracle. Strip just that
	 * oracle-only prefix so diagnostics retain the public function name. */
	if (strncmp(log->message, "oracle_", 7) == 0)
		memmove(log->message, log->message + 7, strlen(log->message + 7) + 1);
}

typedef void (*output_update_fn)(obs_output_t *, obs_data_t *);
typedef void (*output_update_get_log)(log_handler_t *, void **);
typedef void (*output_update_set_log)(log_handler_t, void *);
bool oracle_output_update_logs(output_update_fn update, output_update_get_log get_log, output_update_set_log set_log,
			       obs_data_t *incoming);
bool oracle_output_update_logs(output_update_fn update, output_update_get_log get_log, output_update_set_log set_log,
			       obs_data_t *incoming)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct output_update_log r = {0}, c = {0};
	set_log(output_update_capture, &r);
	update(NULL, incoming);
	set_log(output_update_capture, &c);
	oracle_obs_output_update(NULL, incoming);
	set_log(old_handler, old_param);
	return r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
	       strcmp(r.message, "obs_output_update: Null 'output' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}

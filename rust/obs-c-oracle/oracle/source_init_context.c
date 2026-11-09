/* Original unit and real production adapter; controlled dependencies only. */
#include <string.h>
#include "obs-internal.h"

bool oracle_source_context_init(struct obs_context_data *, enum obs_obj_type, obs_data_t *, const char *, const char *,
				obs_data_t *, bool);
bool oracle_source_signal_array(signal_handler_t *, const char **);
bool oracle_obs_source_init_context(obs_source_t *, obs_data_t *, const char *, const char *, obs_data_t *, bool);
#define obs_source_init_context oracle_obs_source_init_context
#define obs_context_data_init oracle_source_context_init
#define signal_handler_add_array oracle_source_signal_array
#include "obs-source-init-context.c.h"
#include "source-init-context-fields.inc"
#undef signal_handler_add_array
#undef obs_context_data_init
#undef obs_source_init_context

static struct obs_source source_context_fixture;
static bool context_init_result, context_register_result, context_private, context_valid;
static unsigned context_calls;
static int context_signal_token, context_data_token, context_hotkey_token;
static obs_data_t *context_data, *context_hotkeys;
static const char *context_name, *context_uuid;

bool oracle_source_context_init(struct obs_context_data *context, enum obs_obj_type type, obs_data_t *settings,
				const char *name, const char *uuid, obs_data_t *hotkeys, bool private)
{
	context_valid = context_valid && context_calls++ == 0 && context == &source_context_fixture.context &&
			type == OBS_OBJ_TYPE_SOURCE && settings == context_data && hotkeys == context_hotkeys &&
			name == context_name && uuid == context_uuid && private == context_private;
	context->signals = (signal_handler_t *)&context_signal_token;
	return context_init_result;
}

bool oracle_source_signal_array(signal_handler_t *handler, const char **signals)
{
	context_valid = context_valid && context_calls++ == 1 && handler == (void *)&context_signal_token;
	for (size_t i = 0; i < sizeof(source_signals) / sizeof(*source_signals); ++i) {
		if (source_signals[i])
			context_valid = context_valid && signals[i] && strcmp(signals[i], source_signals[i]) == 0;
		else
			context_valid = context_valid && signals[i] == NULL;
	}
	return context_register_result;
}

typedef bool (*source_context_function)(obs_source_t *, obs_data_t *, const char *, const char *, obs_data_t *, bool);
bool oracle_source_init_context_check(source_context_function implementation, bool init, bool registration,
				      bool populated, bool private);
bool oracle_source_init_context_check(source_context_function implementation, bool init, bool registration,
				      bool populated, bool private)
{
	context_init_result = init;
	context_register_result = registration;
	context_private = private;
	context_data = populated ? (obs_data_t *)&context_data_token : NULL;
	context_hotkeys = populated ? (obs_data_t *)&context_hotkey_token : NULL;
	context_name = populated ? "source" : NULL;
	context_uuid = populated ? "uuid" : NULL;
	bool expected = false;
	for (unsigned run = 0; run < 2; ++run) {
		memset(&source_context_fixture, 0, sizeof(source_context_fixture));
		context_calls = 0;
		context_valid = true;
		source_context_function function = run ? implementation : oracle_obs_source_init_context;
		bool result = function(&source_context_fixture, context_data, context_name, context_uuid,
				       context_hotkeys, private);
		if (!context_valid || context_calls != (init ? 2u : 1u) ||
		    source_context_fixture.context.signals != (void *)&context_signal_token)
			return false;
		if (run && result != expected)
			return false;
		expected = result;
	}
	return true;
}

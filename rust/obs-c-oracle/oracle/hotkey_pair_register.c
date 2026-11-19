/* The extracted pair registration wrappers and the production field/call
 * adapters over real private owner layouts. Only the unchanged pair engine
 * (register_hotkey_pair_internal) and the public weak-reference getters are
 * replaced, by recorders; the engine recorder calls the weak helper once per
 * half, as the real engine does. State is thread-local. */
#define obs_hotkey_pair_register_encoder oracle_obs_hotkey_pair_register_encoder
#define obs_hotkey_pair_register_output oracle_obs_hotkey_pair_register_output
#define obs_hotkey_pair_register_service oracle_obs_hotkey_pair_register_service
#define obs_hotkey_pair_register_source oracle_obs_hotkey_pair_register_source
#include "obs-internal.h"
#include <stddef.h>
#include <string.h>

#define PAIR_TRACE 64

static _Thread_local struct obs_source pair_source;
static _Thread_local struct obs_output pair_output;
static _Thread_local struct obs_encoder pair_encoder;
static _Thread_local struct obs_service pair_service;
static _Thread_local size_t pair_weak[4];
static _Thread_local size_t pair_result;
static _Thread_local size_t pair_trace[PAIR_TRACE];
static _Thread_local size_t pair_count;
static _Thread_local obs_hotkey_active_func pair_callbacks[2];

static void pair_record(size_t value)
{
	if (pair_count < PAIR_TRACE)
		pair_trace[pair_count++] = value;
}

static obs_weak_source_t *pair_test_source_weak(obs_source_t *owner)
{
	pair_record(11);
	pair_record(owner == &pair_source);
	return (obs_weak_source_t *)&pair_weak[0];
}
static obs_weak_output_t *pair_test_output_weak(obs_output_t *owner)
{
	pair_record(12);
	pair_record(owner == &pair_output);
	return (obs_weak_output_t *)&pair_weak[1];
}
static obs_weak_encoder_t *pair_test_encoder_weak(obs_encoder_t *owner)
{
	pair_record(13);
	pair_record(owner == &pair_encoder);
	return (obs_weak_encoder_t *)&pair_weak[2];
}
static obs_weak_service_t *pair_test_service_weak(obs_service_t *owner)
{
	pair_record(14);
	pair_record(owner == &pair_service);
	return (obs_weak_service_t *)&pair_weak[3];
}

static size_t pair_callback_index(obs_hotkey_active_func func)
{
	if (!func)
		return 0;
	if (func == pair_callbacks[0])
		return 1;
	if (func == pair_callbacks[1])
		return 2;
	return 3;
}

static obs_hotkey_pair_id pair_test_engine(obs_hotkey_registerer_t type, void *registerer, void *(*weak_ref)(void *),
					   struct obs_context_data *context, const char *name0,
					   const char *description0, const char *name1, const char *description1,
					   obs_hotkey_active_func func0, obs_hotkey_active_func func1, void *data0,
					   void *data1)
{
	pair_record(3);
	pair_record((size_t)type);
	pair_record((size_t)registerer);
	pair_record((size_t)context);
	pair_record((size_t)name0);
	pair_record((size_t)description0);
	pair_record((size_t)name1);
	pair_record((size_t)description1);
	pair_record(pair_callback_index(func0));
	pair_record(pair_callback_index(func1));
	pair_record((size_t)data0);
	pair_record((size_t)data1);
	for (size_t half = 0; half < 2; half++) {
		pair_record(4);
		pair_record((size_t)weak_ref(registerer));
	}
	return pair_result;
}

#define obs_source_get_weak_source pair_test_source_weak
#define obs_output_get_weak_output pair_test_output_weak
#define obs_encoder_get_weak_encoder pair_test_encoder_weak
#define obs_service_get_weak_service pair_test_service_weak
#define register_hotkey_pair_internal pair_test_engine
#include "hotkey_pair_register_fields.inc"
#include "hotkey-pair-register.inc"
#undef register_hotkey_pair_internal
#undef obs_service_get_weak_service
#undef obs_encoder_get_weak_encoder
#undef obs_output_get_weak_output
#undef obs_source_get_weak_source

void *oracle_hotkey_pair_register_reset(int kind, size_t result, obs_hotkey_active_func func0,
					obs_hotkey_active_func func1);
void *oracle_hotkey_pair_register_reset(int kind, size_t result, obs_hotkey_active_func func0,
					obs_hotkey_active_func func1)
{
	pair_result = result;
	pair_callbacks[0] = func0;
	pair_callbacks[1] = func1;
	pair_count = 0;
	memset(pair_trace, 0, sizeof(pair_trace));
	switch (kind) {
	case OBS_HOTKEY_REGISTERER_SOURCE:
		return &pair_source;
	case OBS_HOTKEY_REGISTERER_OUTPUT:
		return &pair_output;
	case OBS_HOTKEY_REGISTERER_ENCODER:
		return &pair_encoder;
	case OBS_HOTKEY_REGISTERER_SERVICE:
		return &pair_service;
	default:
		return NULL;
	}
}

size_t oracle_hotkey_pair_register_snapshot(size_t *trace);
size_t oracle_hotkey_pair_register_snapshot(size_t *trace)
{
	memcpy(trace, pair_trace, sizeof(pair_trace));
	return pair_count;
}

/* Context offsets in the real layouts: the recorded context must be the
 * owner's own embedded context. */
void oracle_hotkey_pair_register_offsets(size_t *values);
void oracle_hotkey_pair_register_offsets(size_t *values)
{
	const size_t actual[] = {offsetof(struct obs_source, context),
				 offsetof(struct obs_output, context),
				 offsetof(struct obs_encoder, context),
				 offsetof(struct obs_service, context),
				 sizeof(obs_hotkey_pair_id),
				 OBS_INVALID_HOTKEY_PAIR_ID};
	memcpy(values, actual, sizeof(actual));
}

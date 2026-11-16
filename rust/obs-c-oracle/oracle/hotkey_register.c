/* The extracted wrappers and production adapters over real private owner
 * layouts. Only their engine/lock/weak-reference dependencies are recorded. */
#define obs_hotkey_register_frontend oracle_obs_hotkey_register_frontend
#define obs_hotkey_register_source oracle_obs_hotkey_register_source
#define obs_hotkey_register_output oracle_obs_hotkey_register_output
#define obs_hotkey_register_encoder oracle_obs_hotkey_register_encoder
#define obs_hotkey_register_service oracle_obs_hotkey_register_service
#include "obs-internal.h"
#include <stddef.h>
#include <string.h>

static _Thread_local struct obs_source register_source;
static _Thread_local struct obs_output register_output;
static _Thread_local struct obs_encoder register_encoder;
static _Thread_local struct obs_service register_service;
static _Thread_local size_t register_weak[4];
static _Thread_local bool register_available;
static _Thread_local bool register_held;
static _Thread_local size_t register_result;
static _Thread_local size_t register_trace[32];
static _Thread_local size_t register_count;
static _Thread_local obs_hotkey_func register_callback;

static void register_record(size_t value)
{
	if (register_count < 32)
		register_trace[register_count++] = value;
}

static bool register_test_lock(void)
{
	register_record(1);
	register_held = register_available;
	return register_available;
}

static void register_test_unlock(void)
{
	register_record(2);
	register_record(register_held);
	register_held = false;
}

static obs_weak_source_t *register_test_source_weak(obs_source_t *owner)
{
	register_record(11);
	register_record(owner == &register_source && register_held);
	return (obs_weak_source_t *)&register_weak[0];
}
static obs_weak_output_t *register_test_output_weak(obs_output_t *owner)
{
	register_record(12);
	register_record(owner == &register_output && register_held);
	return (obs_weak_output_t *)&register_weak[1];
}
static obs_weak_encoder_t *register_test_encoder_weak(obs_encoder_t *owner)
{
	register_record(13);
	register_record(owner == &register_encoder && register_held);
	return (obs_weak_encoder_t *)&register_weak[2];
}
static obs_weak_service_t *register_test_service_weak(obs_service_t *owner)
{
	register_record(14);
	register_record(owner == &register_service && register_held);
	return (obs_weak_service_t *)&register_weak[3];
}

#define obs_hotkey_seam_lock register_test_lock
#define obs_hotkey_seam_unlock register_test_unlock
#define obs_source_get_weak_source register_test_source_weak
#define obs_output_get_weak_output register_test_output_weak
#define obs_encoder_get_weak_encoder register_test_encoder_weak
#define obs_service_get_weak_service register_test_service_weak
#include "hotkey_register_fields.inc"

obs_hotkey_id obs_hotkey_register_call(obs_hotkey_registerer_t kind, void *weak, struct obs_context_data *context,
				       const char *name, const char *description, obs_hotkey_func func, void *data);
obs_hotkey_id obs_hotkey_register_call(obs_hotkey_registerer_t kind, void *weak, struct obs_context_data *context,
				       const char *name, const char *description, obs_hotkey_func func, void *data)
{
	register_record(3);
	register_record((size_t)kind);
	register_record((size_t)weak);
	register_record((size_t)context);
	register_record((size_t)name);
	register_record((size_t)description);
	register_record(func == register_callback);
	register_record((size_t)data);
	register_record(register_held);
	return register_result;
}

#include "hotkey_register.inc"

void *oracle_hotkey_register_reset(int kind, bool available, bool private_source, size_t result, obs_hotkey_func func);
void *oracle_hotkey_register_reset(int kind, bool available, bool private_source, size_t result, obs_hotkey_func func)
{
	register_available = available;
	register_held = false;
	register_source.context.private = private_source;
	register_result = result;
	register_callback = func;
	register_count = 0;
	memset(register_trace, 0, sizeof(register_trace));
	switch (kind) {
	case OBS_HOTKEY_REGISTERER_SOURCE:
		return &register_source;
	case OBS_HOTKEY_REGISTERER_OUTPUT:
		return &register_output;
	case OBS_HOTKEY_REGISTERER_ENCODER:
		return &register_encoder;
	case OBS_HOTKEY_REGISTERER_SERVICE:
		return &register_service;
	default:
		return NULL;
	}
}

void oracle_hotkey_register_snapshot(size_t *trace, size_t *count, bool *held);
void oracle_hotkey_register_snapshot(size_t *trace, size_t *count, bool *held)
{
	memcpy(trace, register_trace, sizeof(register_trace));
	*count = register_count;
	*held = register_held;
}

void oracle_hotkey_register_values(size_t *values);
void oracle_hotkey_register_values(size_t *values)
{
	const size_t actual[] = {sizeof(obs_hotkey_id),         _Alignof(obs_hotkey_id),
				 OBS_INVALID_HOTKEY_ID,         OBS_HOTKEY_REGISTERER_FRONTEND,
				 OBS_HOTKEY_REGISTERER_SOURCE,  OBS_HOTKEY_REGISTERER_OUTPUT,
				 OBS_HOTKEY_REGISTERER_ENCODER, OBS_HOTKEY_REGISTERER_SERVICE};
	memcpy(values, actual, sizeof(actual));
}

#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust pair registration entry points. Rust checks
 * NULL before calling any owner adapter. The weak-reference helpers are the
 * original ones: the unchanged pair engine calls them once per half. */
typedef void *(*obs_hotkey_pair_register_weak_ref)(void *ref);

static void *obs_hotkey_pair_register_weak_encoder(void *ref)
{
	return obs_encoder_get_weak_encoder(ref);
}

static void *obs_hotkey_pair_register_weak_output(void *ref)
{
	return obs_output_get_weak_output(ref);
}

static void *obs_hotkey_pair_register_weak_service(void *ref)
{
	return obs_service_get_weak_service(ref);
}

static void *obs_hotkey_pair_register_weak_source(void *ref)
{
	return obs_source_get_weak_source(ref);
}

void obs_hotkey_pair_register_encoder_fields(obs_encoder_t *owner, obs_hotkey_pair_register_weak_ref *weak_ref,
					     struct obs_context_data **context);
void obs_hotkey_pair_register_encoder_fields(obs_encoder_t *owner, obs_hotkey_pair_register_weak_ref *weak_ref,
					     struct obs_context_data **context)
{
	*weak_ref = obs_hotkey_pair_register_weak_encoder;
	*context = &owner->context;
}

void obs_hotkey_pair_register_output_fields(obs_output_t *owner, obs_hotkey_pair_register_weak_ref *weak_ref,
					    struct obs_context_data **context);
void obs_hotkey_pair_register_output_fields(obs_output_t *owner, obs_hotkey_pair_register_weak_ref *weak_ref,
					    struct obs_context_data **context)
{
	*weak_ref = obs_hotkey_pair_register_weak_output;
	*context = &owner->context;
}

void obs_hotkey_pair_register_service_fields(obs_service_t *owner, obs_hotkey_pair_register_weak_ref *weak_ref,
					     struct obs_context_data **context);
void obs_hotkey_pair_register_service_fields(obs_service_t *owner, obs_hotkey_pair_register_weak_ref *weak_ref,
					     struct obs_context_data **context)
{
	*weak_ref = obs_hotkey_pair_register_weak_service;
	*context = &owner->context;
}

void obs_hotkey_pair_register_source_fields(obs_source_t *owner, obs_hotkey_pair_register_weak_ref *weak_ref,
					    struct obs_context_data **context);
void obs_hotkey_pair_register_source_fields(obs_source_t *owner, obs_hotkey_pair_register_weak_ref *weak_ref,
					    struct obs_context_data **context)
{
	*weak_ref = obs_hotkey_pair_register_weak_source;
	*context = &owner->context;
}

/* Narrow call into the unchanged pair engine in obs-hotkey.c.h. */
obs_hotkey_pair_id obs_hotkey_pair_register_call(obs_hotkey_registerer_t type, void *registerer,
						 obs_hotkey_pair_register_weak_ref weak_ref,
						 struct obs_context_data *context, const char *name0,
						 const char *description0, const char *name1, const char *description1,
						 obs_hotkey_active_func func0, obs_hotkey_active_func func1,
						 void *data0, void *data1);
obs_hotkey_pair_id obs_hotkey_pair_register_call(obs_hotkey_registerer_t type, void *registerer,
						 obs_hotkey_pair_register_weak_ref weak_ref,
						 struct obs_context_data *context, const char *name0,
						 const char *description0, const char *name1, const char *description1,
						 obs_hotkey_active_func func0, obs_hotkey_active_func func1,
						 void *data0, void *data1)
{
	return register_hotkey_pair_internal(type, registerer, weak_ref, context, name0, description0, name1,
					     description1, func0, func1, data0, data1);
}

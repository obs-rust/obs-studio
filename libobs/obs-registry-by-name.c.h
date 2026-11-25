#include "libobs/build/libobs.pch.h"

/* Private helpers are defined in obs.c.h in this unity group. */
static inline void *get_context_by_name(void *vfirst, const char *name, pthread_mutex_t *mutex,
					void *(*addref)(void *));
static inline void *obs_output_addref_safe_(void *ref);
static inline void *obs_encoder_addref_safe_(void *ref);
static inline void *obs_service_addref_safe_(void *ref);

obs_output_t *obs_get_output_by_name(const char *name)
{
	return get_context_by_name(&obs->data.first_output, name, &obs->data.outputs_mutex, obs_output_addref_safe_);
}

obs_encoder_t *obs_get_encoder_by_name(const char *name)
{
	return get_context_by_name(&obs->data.first_encoder, name, &obs->data.encoders_mutex, obs_encoder_addref_safe_);
}

obs_service_t *obs_get_service_by_name(const char *name)
{
	return get_context_by_name(&obs->data.first_service, name, &obs->data.services_mutex, obs_service_addref_safe_);
}

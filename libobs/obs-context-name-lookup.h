/* Private shared name lookup engine for context registry getters. */
#pragma once

static inline void *get_context_by_name(void *vfirst, const char *name, pthread_mutex_t *mutex, void *(*addref)(void *))
{
	struct obs_context_data **first = vfirst;
	struct obs_context_data *context;

	pthread_mutex_lock(mutex);

	/* If context list head has a hash table, look the name up in there */
	if (*first && (*first)->hh.tbl) {
		HASH_FIND_STR(*first, name, context);
	} else {
		context = *first;
		while (context) {
			if (!context->private && strcmp(context->name, name) == 0) {
				break;
			}

			context = context->next;
		}
	}

	if (context)
		addref(context);

	pthread_mutex_unlock(mutex);
	return context;
}

static inline void *obs_output_addref_safe_(void *ref)
{
	return obs_output_get_ref(ref);
}

static inline void *obs_encoder_addref_safe_(void *ref)
{
	return obs_encoder_get_ref(ref);
}

static inline void *obs_service_addref_safe_(void *ref)
{
	return obs_service_get_ref(ref);
}

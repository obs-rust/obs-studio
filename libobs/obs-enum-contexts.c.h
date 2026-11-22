#include "libobs/build/libobs.pch.h"

typedef struct obs_context_data *(*obs_enum_get_ref_t)(struct obs_context_data *);
typedef void (*obs_enum_release_t)(struct obs_context_data *);

static struct obs_context_data *obs_output_enum_get_ref(struct obs_context_data *context)
{
	return (struct obs_context_data *)obs_output_get_ref((obs_output_t *)context);
}

static void obs_output_enum_release(struct obs_context_data *context)
{
	obs_output_release((obs_output_t *)context);
}

static struct obs_context_data *obs_encoder_enum_get_ref(struct obs_context_data *context)
{
	return (struct obs_context_data *)obs_encoder_get_ref((obs_encoder_t *)context);
}

static void obs_encoder_enum_release(struct obs_context_data *context)
{
	obs_encoder_release((obs_encoder_t *)context);
}

static struct obs_context_data *obs_service_enum_get_ref(struct obs_context_data *context)
{
	return (struct obs_context_data *)obs_service_get_ref((obs_service_t *)context);
}

static void obs_service_enum_release(struct obs_context_data *context)
{
	obs_service_release((obs_service_t *)context);
}

/* Walks a snapshot of the list taken under the list mutex, holding a reference
 * on every entry. The callback may therefore release the reference it owns for
 * the current entry: an entry released during the walk is destroyed when the
 * snapshot reference is dropped after the walk, on the enumerating thread. */
static inline void obs_enum(void *pstart, pthread_mutex_t *mutex, bool (*enum_proc)(void *, void *), void *param,
			    obs_enum_get_ref_t get_ref, obs_enum_release_t release)
{
	struct obs_context_data **start = pstart, *context;
	DARRAY(struct obs_context_data *) snapshot;

	assert(start);
	assert(mutex);
	assert(enum_proc);
	assert(get_ref);
	assert(release);

	da_init(snapshot);

	pthread_mutex_lock(mutex);

	context = *start;
	while (context) {
		struct obs_context_data *ref = get_ref(context);
		if (ref)
			da_push_back(snapshot, &ref);
		context = context->next;
	}

	pthread_mutex_unlock(mutex);

	for (size_t i = 0; i < snapshot.num; i++) {
		context = snapshot.array[i];

		pthread_mutex_lock(mutex);
		bool keep_going = enum_proc(param, context);
		pthread_mutex_unlock(mutex);

		if (!keep_going)
			break;
	}

	for (size_t i = 0; i < snapshot.num; i++)
		release(snapshot.array[i]);
	da_free(snapshot);
}

void obs_enum_outputs(bool (*enum_proc)(void *, obs_output_t *), void *param)
{
	obs_enum(&obs->data.first_output, &obs->data.outputs_mutex, (bool (*)(void *, void *))enum_proc, param,
		 obs_output_enum_get_ref, obs_output_enum_release);
}

void obs_enum_encoders(bool (*enum_proc)(void *, obs_encoder_t *), void *param)
{
	obs_enum(&obs->data.first_encoder, &obs->data.encoders_mutex, (bool (*)(void *, void *))enum_proc, param,
		 obs_encoder_enum_get_ref, obs_encoder_enum_release);
}

void obs_enum_services(bool (*enum_proc)(void *, obs_service_t *), void *param)
{
	obs_enum(&obs->data.first_service, &obs->data.services_mutex, (bool (*)(void *, void *))enum_proc, param,
		 obs_service_enum_get_ref, obs_service_enum_release);
}

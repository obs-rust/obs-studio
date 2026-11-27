#include "libobs/build/libobs.pch.h"

/* What obs_source_filter_add() reads, writes, signals and calls (the
 * obs-source shim). The caller rejects a NULL source and filter, keeps both
 * alive, touches the list only between lock and unlock, at indexes below a
 * count read under the lock, and writes the filter's links only once it
 * holds the reference obs_source_filter_add_get_ref() returned; each call is
 * one read, one write or one libobs call. */
void obs_source_filter_add_lock(obs_source_t *source);
void obs_source_filter_add_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->filter_mutex);
}

size_t obs_source_filter_add_count(const obs_source_t *source);
size_t obs_source_filter_add_count(const obs_source_t *source)
{
	return source->filters.num;
}

obs_source_t *obs_source_filter_add_at(const obs_source_t *source, size_t index);
obs_source_t *obs_source_filter_add_at(const obs_source_t *source, size_t index)
{
	return source->filters.array[index];
}

bool obs_source_filter_add_has_parent(const obs_source_t *filter);
bool obs_source_filter_add_has_parent(const obs_source_t *filter)
{
	return filter->filter_parent != NULL;
}

void obs_source_filter_add_warn_present(void);
void obs_source_filter_add_warn_present(void)
{
	blog(LOG_WARNING, "Tried to add a filter that was already "
			  "present on the source");
}

bool obs_source_filter_add_owns_info_id(const obs_source_t *source);
bool obs_source_filter_add_owns_info_id(const obs_source_t *source)
{
	return source->owns_info_id;
}

uint32_t obs_source_filter_add_output_flags(const obs_source_t *source);
uint32_t obs_source_filter_add_output_flags(const obs_source_t *source)
{
	return source->info.output_flags;
}

obs_source_t *obs_source_filter_add_get_ref(obs_source_t *filter);
obs_source_t *obs_source_filter_add_get_ref(obs_source_t *filter)
{
	return obs_source_get_ref(filter);
}

void obs_source_filter_add_set_parent(obs_source_t *filter, obs_source_t *source);
void obs_source_filter_add_set_parent(obs_source_t *filter, obs_source_t *source)
{
	filter->filter_parent = source;
}

void obs_source_filter_add_set_target(obs_source_t *filter, obs_source_t *target);
void obs_source_filter_add_set_target(obs_source_t *filter, obs_source_t *target)
{
	filter->filter_target = target;
}

void obs_source_filter_add_insert_front(obs_source_t *source, obs_source_t *filter);
void obs_source_filter_add_insert_front(obs_source_t *source, obs_source_t *filter)
{
	da_insert(source->filters, 0, &filter);
}

void obs_source_filter_add_unlock(obs_source_t *source);
void obs_source_filter_add_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->filter_mutex);
}

void obs_source_filter_add_signal(obs_source_t *source, obs_source_t *filter);
void obs_source_filter_add_signal(obs_source_t *source, obs_source_t *filter)
{
	struct calldata cd;
	uint8_t stack[128];

	calldata_init_fixed(&cd, stack, sizeof(stack));
	calldata_set_ptr(&cd, "source", source);
	calldata_set_ptr(&cd, "filter", filter);

	signal_handler_signal(obs->signals, "source_filter_add", &cd);
	signal_handler_signal(source->context.signals, "filter_add", &cd);
}

void obs_source_filter_add_log(obs_source_t *source, obs_source_t *filter);
void obs_source_filter_add_log(obs_source_t *source, obs_source_t *filter)
{
	blog(LOG_DEBUG, "- filter '%s' (%s) added to source '%s'", filter->context.name, filter->info.id,
	     source->context.name);
}

bool obs_source_filter_add_has_callback(const obs_source_t *filter);
bool obs_source_filter_add_has_callback(const obs_source_t *filter)
{
	return filter->info.filter_add != NULL;
}

void obs_source_filter_add_callback(obs_source_t *filter);
void obs_source_filter_add_callback(obs_source_t *filter)
{
	filter->info.filter_add(filter->context.data, filter->filter_parent);
}

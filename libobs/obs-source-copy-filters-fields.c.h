#include "libobs/build/libobs.pch.h"

/* What obs_source_copy_filters(), obs_source_copy_single_filter() and
 * obs_source_duplicate_filters() read and call (the obs-source shim). The
 * caller rejects a NULL destination, source or filter where the original
 * did, keeps the sources alive, and reads the filter list only between lock
 * and unlock, at indexes below a count read under the lock; each call is one
 * read or one libobs call. */
bool obs_source_copy_filters_private(const obs_source_t *dst);
bool obs_source_copy_filters_private(const obs_source_t *dst)
{
	return dst->context.private;
}

void obs_source_copy_filters_lock(obs_source_t *src);
void obs_source_copy_filters_lock(obs_source_t *src)
{
	pthread_mutex_lock(&src->filter_mutex);
}

size_t obs_source_copy_filters_count(const obs_source_t *src);
size_t obs_source_copy_filters_count(const obs_source_t *src)
{
	return src->filters.num;
}

obs_source_t *obs_source_copy_filters_at(const obs_source_t *src, size_t index);
obs_source_t *obs_source_copy_filters_at(const obs_source_t *src, size_t index)
{
	return src->filters.array[index];
}

obs_source_t *obs_source_copy_filters_get_ref(obs_source_t *filter);
obs_source_t *obs_source_copy_filters_get_ref(obs_source_t *filter)
{
	return obs_source_get_ref(filter);
}

void obs_source_copy_filters_unlock(obs_source_t *src);
void obs_source_copy_filters_unlock(obs_source_t *src)
{
	pthread_mutex_unlock(&src->filter_mutex);
}

bool obs_source_copy_filters_compatible(obs_source_t *dst, obs_source_t *filter);
bool obs_source_copy_filters_compatible(obs_source_t *dst, obs_source_t *filter)
{
	return filter_compatible(dst, filter);
}

char *obs_source_copy_filters_new_name(obs_source_t *dst, obs_source_t *filter);
char *obs_source_copy_filters_new_name(obs_source_t *dst, obs_source_t *filter)
{
	return get_new_filter_name(dst, filter->context.name);
}

bool obs_source_copy_filters_enabled(const obs_source_t *filter);
bool obs_source_copy_filters_enabled(const obs_source_t *filter)
{
	return obs_source_enabled(filter);
}

obs_source_t *obs_source_copy_filters_duplicate(obs_source_t *filter, const char *name, bool private);
obs_source_t *obs_source_copy_filters_duplicate(obs_source_t *filter, const char *name, bool private)
{
	return obs_source_duplicate(filter, name, private);
}

void obs_source_copy_filters_set_enabled(obs_source_t *copy, bool enabled);
void obs_source_copy_filters_set_enabled(obs_source_t *copy, bool enabled)
{
	obs_source_set_enabled(copy, enabled);
}

void obs_source_copy_filters_free_name(char *name);
void obs_source_copy_filters_free_name(char *name)
{
	bfree(name);
}

void obs_source_copy_filters_add(obs_source_t *dst, obs_source_t *copy);
void obs_source_copy_filters_add(obs_source_t *dst, obs_source_t *copy)
{
	obs_source_filter_add(dst, copy);
}

void obs_source_copy_filters_release(obs_source_t *source);
void obs_source_copy_filters_release(obs_source_t *source)
{
	obs_source_release(source);
}

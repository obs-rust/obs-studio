#include "libobs/build/libobs.pch.h"

void obs_source_duplicate_filters(obs_source_t *dst, obs_source_t *src, bool private)
{
	DARRAY(obs_source_t *) filters;

	da_init(filters);

	pthread_mutex_lock(&src->filter_mutex);
	da_reserve(filters, src->filters.num);
	for (size_t i = 0; i < src->filters.num; i++) {
		obs_source_t *s = obs_source_get_ref(src->filters.array[i]);
		if (s)
			da_push_back(filters, &s);
	}
	pthread_mutex_unlock(&src->filter_mutex);

	for (size_t i = filters.num; i > 0; i--) {
		obs_source_t *src_filter = filters.array[i - 1];
		char *new_name = get_new_filter_name(dst, src_filter->context.name);
		bool enabled = obs_source_enabled(src_filter);

		obs_source_t *dst_filter = obs_source_duplicate(src_filter, new_name, private);
		obs_source_set_enabled(dst_filter, enabled);

		bfree(new_name);
		obs_source_filter_add(dst, dst_filter);
		obs_source_release(dst_filter);
		obs_source_release(src_filter);
	}

	da_free(filters);
}

void obs_source_copy_filters(obs_source_t *dst, obs_source_t *src)
{
	if (!obs_source_valid(dst, "obs_source_copy_filters"))
		return;
	if (!obs_source_valid(src, "obs_source_copy_filters"))
		return;

	obs_source_duplicate_filters(dst, src, dst->context.private);
}

static void duplicate_filter(obs_source_t *dst, obs_source_t *filter)
{
	if (!filter_compatible(dst, filter))
		return;

	char *new_name = get_new_filter_name(dst, filter->context.name);
	bool enabled = obs_source_enabled(filter);

	obs_source_t *dst_filter = obs_source_duplicate(filter, new_name, true);
	obs_source_set_enabled(dst_filter, enabled);

	bfree(new_name);
	obs_source_filter_add(dst, dst_filter);
	obs_source_release(dst_filter);
}

void obs_source_copy_single_filter(obs_source_t *dst, obs_source_t *filter)
{
	if (!obs_source_valid(dst, "obs_source_copy_single_filter"))
		return;
	if (!obs_source_valid(filter, "obs_source_copy_single_filter"))
		return;

	duplicate_filter(dst, filter);
}

#include "libobs/build/libobs.pch.h"

/* The original obs-source.c.h data_valid guard, scoped to this extraction. */
#define data_valid(source, f) (obs_source_valid(source, f) && (source)->context.data)

void obs_source_save(obs_source_t *source)
{
	if (!data_valid(source, "obs_source_save"))
		return;

	obs_source_dosignal(source, "source_save", "save");

	if (source->info.save)
		source->info.save(source->context.data, source->context.settings);
}

void obs_source_load(obs_source_t *source)
{
	if (!data_valid(source, "obs_source_load"))
		return;
	if (source->info.load)
		source->info.load(source->context.data, source->context.settings);

	obs_source_dosignal(source, "source_load", "load");
}

void obs_source_load2(obs_source_t *source)
{
	if (!data_valid(source, "obs_source_load2"))
		return;

	obs_source_load(source);

	for (size_t i = source->filters.num; i > 0; i--) {
		obs_source_t *filter = source->filters.array[i - 1];
		obs_source_load(filter);
	}
}

#undef data_valid

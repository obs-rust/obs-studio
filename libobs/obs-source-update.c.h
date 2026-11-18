#include "libobs/build/libobs.pch.h"

void obs_source_update(obs_source_t *source, obs_data_t *settings)
{
	if (!obs_source_valid(source, "obs_source_update"))
		return;

	if (settings) {
		obs_data_apply(source->context.settings, settings);
	}

	if (source->info.output_flags & OBS_SOURCE_VIDEO) {
		os_atomic_inc_long(&source->defer_update_count);
	} else if (source->context.data && source->info.update) {
		source->info.update(source->context.data, source->context.settings);
		obs_source_dosignal(source, "source_update", "update");
	}
}

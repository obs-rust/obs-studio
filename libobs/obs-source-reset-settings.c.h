#include "libobs/build/libobs.pch.h"

void obs_source_reset_settings(obs_source_t *source, obs_data_t *settings)
{
	if (!obs_source_valid(source, "obs_source_reset_settings"))
		return;

	obs_data_clear(source->context.settings);
	obs_source_update(source, settings);
}

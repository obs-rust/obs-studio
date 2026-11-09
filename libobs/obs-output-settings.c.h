#include "libobs/build/libobs.pch.h"

obs_data_t *obs_output_get_settings(const obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_get_settings"))
		return NULL;

	obs_data_addref(output->context.settings);
	return output->context.settings;
}

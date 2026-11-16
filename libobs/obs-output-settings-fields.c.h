#include "libobs/build/libobs.pch.h"

obs_data_t *const *obs_output_settings_fields(const obs_output_t *output)
{
	return &output->context.settings;
}

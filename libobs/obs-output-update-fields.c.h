#include "libobs/build/libobs.pch.h"

/* Live adapter for obs_output_update() in rust/obs-output. Called only after
 * Rust checks output; each function reads what the C read at that point. */
obs_data_t *obs_output_update_settings(const obs_output_t *output);
obs_data_t *obs_output_update_settings(const obs_output_t *output)
{
	return output->context.settings;
}

bool obs_output_update_has_callback(const obs_output_t *output);
bool obs_output_update_has_callback(const obs_output_t *output)
{
	return output->info.update != NULL;
}

void obs_output_update_call(obs_output_t *output);
void obs_output_update_call(obs_output_t *output)
{
	output->info.update(output->context.data, output->context.settings);
}

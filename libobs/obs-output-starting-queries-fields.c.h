#include "libobs/build/libobs.pch.h"

/* Keep C ownership of the real private layout and atomic operations. */
bool obs_output_starting_load_active(const struct obs_output *output);
bool obs_output_starting_load_starting(const struct obs_output *output);
bool obs_output_starting_load_reconnecting(const struct obs_output *output);
bool obs_output_starting_load_delay_active(const struct obs_output *output);

bool obs_output_starting_load_active(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->active);
}

bool obs_output_starting_load_starting(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->starting);
}

bool obs_output_starting_load_reconnecting(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->reconnecting);
}

bool obs_output_starting_load_delay_active(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->delay_active);
}

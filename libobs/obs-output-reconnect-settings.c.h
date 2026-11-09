#include "libobs/build/libobs.pch.h"

void obs_output_set_reconnect_settings(obs_output_t *output, int retry_count, int retry_sec)
{
	if (!obs_output_valid(output, "obs_output_set_reconnect_settings"))
		return;

	output->reconnect_retry_max = retry_count;
	output->reconnect_retry_sec = retry_sec;
}

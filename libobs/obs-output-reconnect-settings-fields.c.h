#include "libobs/build/libobs.pch.h"

/* Private borrowed addresses; the output layout stays owned by C. */
struct obs_output_reconnect_settings_fields {
	int *retry_max;
	int *retry_sec;
};

struct obs_output_reconnect_settings_fields obs_output_reconnect_settings_fields(obs_output_t *output);
struct obs_output_reconnect_settings_fields obs_output_reconnect_settings_fields(obs_output_t *output)
{
	return (struct obs_output_reconnect_settings_fields){&output->reconnect_retry_max,
							     &output->reconnect_retry_sec};
}

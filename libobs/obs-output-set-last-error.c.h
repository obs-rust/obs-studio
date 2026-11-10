#include "libobs/build/libobs-ported-service-getters.pch.h"

void obs_output_set_last_error(obs_output_t *output, const char *message)
{
	if (!obs_output_valid(output, "obs_output_set_last_error"))
		return;

	if (output->last_error_message)
		bfree(output->last_error_message);

	if (message)
		output->last_error_message = bstrdup(message);
	else
		output->last_error_message = NULL;
}

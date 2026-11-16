#include "libobs/build/libobs.pch.h"

char **obs_output_last_error_fields(obs_output_t *output)
{
	return &output->last_error_message;
}

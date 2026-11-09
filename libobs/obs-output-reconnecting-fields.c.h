#include "libobs/build/libobs.pch.h"

struct obs_output_reconnecting_fields obs_output_reconnecting_fields(const obs_output_t *output)
{
	return (struct obs_output_reconnecting_fields){output ? &output->reconnecting : NULL};
}

#include "libobs/build/libobs.pch.h"

struct obs_output_flags_fields obs_output_flags_fields(const obs_output_t *output)
{
	return (struct obs_output_flags_fields){&output->info.flags};
}

#include "libobs/build/libobs.pch.h"

/* Defined later by obs-output.c.h in the core-output unity group. */
static inline bool delay_active(const struct obs_output *output);
static inline bool delay_capturing(const struct obs_output *output);

uint64_t obs_output_get_total_bytes(const obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_get_total_bytes"))
		return 0;
	if (!output->info.get_total_bytes)
		return 0;

	if (delay_active(output) && !delay_capturing(output))
		return 0;

	return output->info.get_total_bytes(output->context.data);
}

#include "libobs/build/libobs.pch.h"

/* Defined later by obs-output.c.h in the core-output unity group. */
static inline bool log_flag_video(const struct obs_output *output, const char *func_name);

void obs_output_set_preferred_size(obs_output_t *output, uint32_t width, uint32_t height)
{
	if (!obs_output_valid(output, "obs_output_set_preferred_size"))
		return;
	if (!log_flag_video(output, __func__))
		return;

	obs_output_set_preferred_size2(output, width, height, 0);
}

#include "libobs/build/libobs.pch.h"

/* Original helpers retained only for the isolated C reference. */
static inline bool flag_encoded(const struct obs_output *output)
{
	return (output->info.flags & OBS_OUTPUT_ENCODED) != 0;
}
static inline bool log_flag_video(const struct obs_output *output, const char *func_name)
{
	bool ret = (output->info.flags & OBS_OUTPUT_VIDEO) != 0;
	if (!ret)
		blog(LOG_WARNING, "Output '%s': Tried to use %s on a non-video output", output->context.name,
		     func_name);
	return ret;
}

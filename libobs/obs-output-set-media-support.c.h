#include "libobs/build/libobs-ported-service-getters.pch.h"

/* Original flag helpers for the isolated C reference and oracle. */
static inline bool flag_encoded(const struct obs_output *output)
{
	return (output->info.flags & OBS_OUTPUT_ENCODED) != 0;
}

static inline bool log_flag_encoded(const struct obs_output *output, const char *func_name, bool inverse_log)
{
	const char *prefix = inverse_log ? "n encoded" : " raw";
	bool ret = flag_encoded(output);
	if ((!inverse_log && !ret) || (inverse_log && ret))
		blog(LOG_WARNING, "Output '%s': Tried to use %s on a%s output", output->context.name, func_name,
		     prefix);
	return ret;
}

static inline bool flag_video(const struct obs_output *output)
{
	return (output->info.flags & OBS_OUTPUT_VIDEO) != 0;
}

static inline bool flag_audio(const struct obs_output *output)
{
	return (output->info.flags & OBS_OUTPUT_AUDIO) != 0;
}

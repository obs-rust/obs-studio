#include "libobs/build/libobs.pch.h"

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

static inline bool log_flag_video(const struct obs_output *output, const char *func_name)
{
	bool ret = flag_video(output);
	if (!ret)
		blog(LOG_WARNING, "Output '%s': Tried to use %s on a non-video output", output->context.name,
		     func_name);
	return ret;
}

static inline bool flag_audio(const struct obs_output *output)
{
	return (output->info.flags & OBS_OUTPUT_AUDIO) != 0;
}

static inline bool log_flag_audio(const struct obs_output *output, const char *func_name)
{
	bool ret = flag_audio(output);
	if (!ret)
		blog(LOG_WARNING, "Output '%s': Tried to use %s on a non-audio output", output->context.name,
		     func_name);
	return ret;
}

void obs_output_set_video_conversion(obs_output_t *output, const struct video_scale_info *conversion)
{
	if (!obs_output_valid(output, "obs_output_set_video_conversion"))
		return;
	if (!obs_ptr_valid(conversion, "obs_output_set_video_conversion"))
		return;
	if (log_flag_encoded(output, __func__, true) || !log_flag_video(output, __func__))
		return;

	output->video_conversion = *conversion;
	output->video_conversion_set = true;
}

void obs_output_set_audio_conversion(obs_output_t *output, const struct audio_convert_info *conversion)
{
	if (!obs_output_valid(output, "obs_output_set_audio_conversion"))
		return;
	if (!obs_ptr_valid(conversion, "obs_output_set_audio_conversion"))
		return;
	if (log_flag_encoded(output, __func__, true) || !log_flag_audio(output, __func__))
		return;

	output->audio_conversion = *conversion;
	output->audio_conversion_set = true;
}

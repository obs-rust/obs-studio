#include "libobs/build/libobs.pch.h"

/* Defined later by obs-output.c.h in the core-output unity group. */
static inline bool log_flag_encoded(const struct obs_output *output, const char *func_name, bool inverse_log);
static inline bool log_flag_video(const struct obs_output *output, const char *func_name);
static inline bool log_flag_audio(const struct obs_output *output, const char *func_name);

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

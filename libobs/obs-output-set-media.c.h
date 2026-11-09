#include "libobs/build/libobs.pch.h"

/* Defined by obs-output.c.h later in the core-output unity group. */
static inline bool log_flag_encoded(const struct obs_output *output, const char *func_name, bool inverse_log);
static inline bool flag_video(const struct obs_output *output);
static inline bool flag_audio(const struct obs_output *output);

void obs_output_set_media(obs_output_t *output, video_t *video, audio_t *audio)
{
	if (!obs_output_valid(output, "obs_output_set_media"))
		return;
	if (log_flag_encoded(output, __func__, true))
		return;

	if (flag_video(output))
		output->video = video;
	if (flag_audio(output))
		output->audio = audio;
}

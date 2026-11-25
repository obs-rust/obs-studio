#include "libobs/build/libobs.pch.h"

/* Defined by obs-output.c.h in the same core-output unity group. */
static inline bool active(const struct obs_output *output);
static inline bool delay_active(const struct obs_output *output);
static inline bool log_flag_encoded(const struct obs_output *output, const char *func_name, bool inverse_log);

static inline bool initialize_audio_encoders(obs_output_t *output)
{
	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		obs_encoder_t *audio = output->audio_encoders[i];

		if (audio && !obs_encoder_initialize(audio)) {
			obs_output_set_last_error(output, obs_encoder_get_last_error(audio));
			return false;
		}
	}

	return true;
}

static inline bool initialize_video_encoders(obs_output_t *output)
{
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		obs_encoder_t *video = output->video_encoders[i];

		if (video && !obs_encoder_initialize(video)) {
			obs_output_set_last_error(output, obs_encoder_get_last_error(video));
			return false;
		}
	}

	return true;
}

bool obs_output_initialize_encoders(obs_output_t *output, uint32_t flags)
{
	UNUSED_PARAMETER(flags);

	if (!obs_output_valid(output, "obs_output_initialize_encoders"))
		return false;
	if (!log_flag_encoded(output, __func__, false))
		return false;
	if (active(output))
		return delay_active(output);

	if (obs_output_start_init_video(output->info.flags) && !initialize_video_encoders(output))
		return false;
	if (obs_output_start_init_audio(output->info.flags) && !initialize_audio_encoders(output))
		return false;

	return true;
}

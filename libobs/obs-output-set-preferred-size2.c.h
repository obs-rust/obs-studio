#include "libobs/build/libobs.pch.h"

static inline bool active(const struct obs_output *output);
static inline bool flag_encoded(const struct obs_output *output);
static inline bool log_flag_video(const struct obs_output *output, const char *func_name);

void obs_output_set_preferred_size2(obs_output_t *output, uint32_t width, uint32_t height, size_t idx)
{
	if (!obs_output_valid(output, "obs_output_set_preferred_size2"))
		return;
	if (!log_flag_video(output, __func__))
		return;
	if (!obs_output_slots_video_slot(idx))
		return;

	if (active(output)) {
		blog(LOG_WARNING,
		     "output '%s': Cannot set the preferred "
		     "resolution while the output is active",
		     obs_output_get_name(output));
		return;
	}

	// Used for raw video output
	if (obs_output_slots_default_index(idx)) {
		output->scaled_width = width;
		output->scaled_height = height;
	}

	if (flag_encoded(output)) {
		if (output->video_encoders[idx])
			obs_encoder_set_scaled_size(output->video_encoders[idx], width, height);
	}
}

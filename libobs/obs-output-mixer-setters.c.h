#include "libobs/build/libobs.pch.h"

/* Defined by obs-output.c.h later in the core-output unity group. */
static inline bool active(const struct obs_output *output);
static inline bool log_flag_encoded(const struct obs_output *output, const char *func_name, bool inverse_log);

void obs_output_set_mixer(obs_output_t *output, size_t mixer_idx)
{
	if (!obs_output_valid(output, "obs_output_set_mixer"))
		return;
	if (log_flag_encoded(output, __func__, true))
		return;
	if (!obs_output_slots_mixer_in_range(mixer_idx)) {
		blog(LOG_WARNING, "output '%s': Ignoring mixer index %zu, it must be below %d",
		     obs_output_get_name(output), mixer_idx, MAX_AUDIO_MIXES);
		return;
	}
	if (active(output))
		return;

	output->mixer_mask = obs_output_slots_mixer_mask(mixer_idx);
}

void obs_output_set_mixers(obs_output_t *output, size_t mixers)
{
	if (!obs_output_valid(output, "obs_output_set_mixers"))
		return;
	if (log_flag_encoded(output, __func__, true))
		return;
	if (active(output))
		return;

	output->mixer_mask = mixers;
}

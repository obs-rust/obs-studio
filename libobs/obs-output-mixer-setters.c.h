#include "libobs/build/libobs-ported-output-mixer-setters.pch.h"

static inline bool active(const struct obs_output *output)
{
	return os_atomic_load_bool(&output->active);
}
static inline bool log_flag_encoded(const struct obs_output *output, const char *func_name, bool inverse_log)
{
	const char *prefix = inverse_log ? "n encoded" : " raw";
	bool ret = (output->info.flags & OBS_OUTPUT_ENCODED) != 0;
	if ((!inverse_log && !ret) || (inverse_log && ret))
		blog(LOG_WARNING, "Output '%s': Tried to use %s on a%s output", output->context.name, func_name,
		     prefix);
	return ret;
}

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

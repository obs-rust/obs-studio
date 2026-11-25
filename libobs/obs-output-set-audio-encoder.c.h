#include "libobs/build/libobs.pch.h"

/* Unity-local helpers remain in obs-output.c.h, later in this group. */
static inline bool active(const struct obs_output *output);
static inline bool log_flag_encoded(const struct obs_output *output, const char *func_name, bool inverse_log);
static inline bool log_flag_audio(const struct obs_output *output, const char *func_name);

void obs_output_set_audio_encoder(obs_output_t *output, obs_encoder_t *encoder, size_t idx)
{
	if (!obs_output_valid(output, "obs_output_set_audio_encoder"))
		return;
	if (!log_flag_encoded(output, __func__, false) || !log_flag_audio(output, __func__))
		return;
	if (encoder && !obs_output_slots_audio_type_ok((int)encoder->info.type)) {
		blog(LOG_WARNING, "obs_output_set_audio_encoder: "
				  "encoder passed is not an audio encoder");
		return;
	}
	if (active(output)) {
		blog(LOG_WARNING,
		     "%s: tried to set audio encoder %d on output \"%s\" "
		     "while the output is still active!",
		     __func__, (int)idx, output->context.name);
		return;
	}

	if (!obs_output_slots_audio_index_ok(output->info.flags, idx))
		return;

	if (output->audio_encoders[idx] == encoder)
		return;

	obs_encoder_remove_output(output->audio_encoders[idx], output);
	obs_encoder_release(output->audio_encoders[idx]);

	output->audio_encoders[idx] = obs_encoder_get_ref(encoder);
	obs_encoder_add_output(output->audio_encoders[idx], output);
}

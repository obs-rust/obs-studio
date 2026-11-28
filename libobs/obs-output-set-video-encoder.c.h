#include "libobs/build/libobs.pch.h"

/* Unity-local helpers remain in obs-output.c.h, later in this group. */
static inline bool active(const struct obs_output *output);
static inline bool log_flag_encoded(const struct obs_output *output, const char *func_name, bool inverse_log);
static inline bool log_flag_video(const struct obs_output *output, const char *func_name);
static void destroy_caption_track(struct caption_track_data **ctrack_ptr);
static struct caption_track_data *create_caption_track(void);

void obs_output_set_video_encoder2(obs_output_t *output, obs_encoder_t *encoder, size_t idx)
{
	if (!obs_output_valid(output, "obs_output_set_video_encoder2"))
		return;
	if (!log_flag_encoded(output, __func__, false) || !log_flag_video(output, __func__))
		return;
	if (encoder && !obs_output_slots_video_type_ok((int)encoder->info.type)) {
		blog(LOG_WARNING, "obs_output_set_video_encoder: "
				  "encoder passed is not a video encoder");
		return;
	}
	if (active(output)) {
		blog(LOG_WARNING,
		     "%s: tried to set video encoder on output \"%s\" "
		     "while the output is still active!",
		     __func__, output->context.name);
		return;
	}

	if (!obs_output_slots_video_index_ok(output->info.flags, idx))
		return;

	if (output->video_encoders[idx] == encoder)
		return;

	obs_encoder_remove_output(output->video_encoders[idx], output);
	obs_encoder_release(output->video_encoders[idx]);

	output->video_encoders[idx] = obs_encoder_get_ref(encoder);
	obs_encoder_add_output(output->video_encoders[idx], output);

	/* Decide on the stored slot, not the argument: the reference fails
	 * for an encoder that is being destroyed, and the slot stays empty. */
	destroy_caption_track(&output->caption_tracks[idx]);
	if (output->video_encoders[idx] != NULL) {
		output->caption_tracks[idx] = create_caption_track();
	} else {
		output->caption_tracks[idx] = NULL;
	}

	// Set preferred resolution on the default index to preserve old behavior
	if (obs_output_slots_default_index(idx) && output->video_encoders[idx]) {
		/* set the preferred resolution on the encoder */
		if (obs_output_slots_scaled_axis_set(output->scaled_width) &&
		    obs_output_slots_scaled_axis_set(output->scaled_height))
			obs_encoder_set_scaled_size(output->video_encoders[idx], output->scaled_width,
						    output->scaled_height);
	}
}

void obs_output_set_video_encoder(obs_output_t *output, obs_encoder_t *encoder)
{
	if (!obs_output_valid(output, "obs_output_set_video_encoder"))
		return;

	obs_output_set_video_encoder2(output, encoder, 0);
}

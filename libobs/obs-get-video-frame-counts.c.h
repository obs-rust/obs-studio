#include "libobs/build/libobs.pch.h"

/* An encoder that rescales on the GPU encodes from an encoder-only mix of
 * the same view (maybe_set_up_gpu_rescale() in obs-encoder.c), and that
 * mix's video_t counts its frames. The mixes are read under mixes_mutex: an
 * encoder-only mix is freed under it when its last encoder stops. */
void obs_get_video_frame_counts(uint32_t *total, uint32_t *skipped)
{
	struct obs_core_video_mix *main_mix = obs->data.main_canvas ? obs->data.main_canvas->mix : NULL;
	uint32_t total_frames = 0;
	uint32_t skipped_frames = 0;

	if (main_mix) {
		pthread_mutex_lock(&obs->video.mixes_mutex);
		for (size_t i = 0; i < obs->video.mixes.num; i++) {
			struct obs_core_video_mix *mix = obs->video.mixes.array[i];
			/* obs_core_setup_rules_mix_step(): each read is made
			 * only when the rule asks for it, as before. */
			enum obs_core_setup_rules_mix step =
				obs_core_setup_rules_mix_step(OBS_CORE_SETUP_RULES_MIX_MAIN, mix == main_mix);
			if (step == OBS_CORE_SETUP_RULES_MIX_ENCODER_ONLY)
				step = obs_core_setup_rules_mix_step(step, mix->encoder_only_mix);
			if (step == OBS_CORE_SETUP_RULES_MIX_VIEW)
				step = obs_core_setup_rules_mix_step(step, mix->view == main_mix->view);
			if (step == OBS_CORE_SETUP_RULES_MIX_VIDEO)
				step = obs_core_setup_rules_mix_step(step, mix->video != NULL);

			if (step != OBS_CORE_SETUP_RULES_MIX_COUNT)
				continue;

			total_frames += video_output_get_total_frames(mix->video);
			skipped_frames += video_output_get_skipped_frames(mix->video);
		}
		pthread_mutex_unlock(&obs->video.mixes_mutex);
	}

	if (total)
		*total = total_frames;
	if (skipped)
		*skipped = skipped_frames;
}

#include "libobs/build/libobs.pch.h"

bool obs_get_audio_info(struct obs_audio_info *oai)
{
	/* no core (before obs_startup): no audio (#2566) */
	if (!obs)
		return false;

	struct obs_core_audio *audio = &obs->audio;
	const struct audio_output_info *info;

	if (!oai || !audio->audio)
		return false;

	info = audio_output_get_info(audio->audio);

	oai->samples_per_sec = info->samples_per_sec;
	oai->speakers = info->speakers;
	return true;
}

bool obs_get_audio_info2(struct obs_audio_info2 *oai2)
{
	/* no core (before obs_startup): no audio (#3363) */
	if (!obs)
		return false;

	struct obs_core_audio *audio = &obs->audio;
	struct obs_audio_info oai;

	if (!obs_get_audio_info(&oai) || !oai2 || !audio->audio) {
		return false;
	} else {
		oai2->samples_per_sec = oai.samples_per_sec;
		oai2->speakers = oai.speakers;
		oai2->fixed_buffering = audio->fixed_buffer;
		oai2->max_buffering_ms =
			(uint32_t)obs_core_setup_rules_buffer_ms(audio->max_buffering_ticks, oai2->samples_per_sec);
		return true;
	}
}

#include "libobs/build/libobs.pch.h"

/* Defined in obs.c.h later in this core unity group. */
static bool obs_init_audio(struct audio_output_info *ai);
static void obs_free_audio(void);

bool obs_reset_audio2(const struct obs_audio_info2 *oai)
{
	struct obs_core_audio *audio;
	struct audio_output_info ai;

	/* no core (before obs_startup): refused before &obs->audio (#3363) */
	if (obs_core_setup_rules_audio_refused(OBS_CORE_SETUP_RULES_AUDIO_CORE, obs != NULL))
		return false;
	audio = &obs->audio;

	/* don't allow changing of audio settings if active. */
	if (audio->audio &&
	    obs_core_setup_rules_audio_refused(OBS_CORE_SETUP_RULES_AUDIO_ACTIVE, audio_output_active(audio->audio)))
		return false;

	obs_free_audio();
	if (!oai)
		return true;

	if (obs_core_setup_rules_buffer_set(oai->max_buffering_ms)) {
		audio->max_buffering_ticks =
			(int)obs_core_setup_rules_buffer_ticks(oai->max_buffering_ms, oai->samples_per_sec);
	} else {
		audio->max_buffering_ticks = (int)obs_core_setup_rules_buffer_default_ticks();
	}
	audio->fixed_buffer = oai->fixed_buffering;

	int max_buffering_ms = obs_core_setup_rules_buffer_ms(audio->max_buffering_ticks, oai->samples_per_sec);

	ai.name = "Audio";
	ai.samples_per_sec = oai->samples_per_sec;
	ai.format = AUDIO_FORMAT_FLOAT_PLANAR;
	ai.speakers = oai->speakers;
	ai.input_callback = audio_callback;

	blog(LOG_INFO, "---------------------------------");
	blog(LOG_INFO,
	     "audio settings reset:\n"
	     "\tsamples per sec: %d\n"
	     "\tspeakers:        %d\n"
	     "\tmax buffering:   %d milliseconds\n"
	     "\tbuffering type:  %s",
	     (int)ai.samples_per_sec, (int)ai.speakers, max_buffering_ms,
	     oai->fixed_buffering ? "fixed" : "dynamically increasing");

	return obs_init_audio(&ai);
}

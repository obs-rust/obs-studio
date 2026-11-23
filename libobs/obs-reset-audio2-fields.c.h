#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_reset_audio2() (#5486): the steps on the
 * core's private audio state. The gates, the buffering ticks and the reset
 * message are the Rust shim's; the shim checks the core before it asks for
 * the output, and asks whether the output is active only when there is one. */

/* Defined in obs.c.h later in this core unity group. */
static bool obs_init_audio(struct audio_output_info *ai);
static void obs_free_audio(void);

bool obs_reset_audio2_core(void);
bool obs_reset_audio2_core(void)
{
	return obs != NULL;
}

bool obs_reset_audio2_has_output(void);
bool obs_reset_audio2_has_output(void)
{
	return obs->audio.audio != NULL;
}

bool obs_reset_audio2_output_active(void);
bool obs_reset_audio2_output_active(void)
{
	return audio_output_active(obs->audio.audio);
}

void obs_reset_audio2_free(void);
void obs_reset_audio2_free(void)
{
	obs_free_audio();
}

void obs_reset_audio2_set_buffering(int max_buffering_ticks, bool fixed_buffer);
void obs_reset_audio2_set_buffering(int max_buffering_ticks, bool fixed_buffer)
{
	struct obs_core_audio *audio = &obs->audio;
	audio->max_buffering_ticks = max_buffering_ticks;
	audio->fixed_buffer = fixed_buffer;
}

/* obs_init_audio() on any info; the C reference's original reaches it here
 * (obs-reset-audio2-reference-steps.c.h). */
bool obs_reset_audio2_open(struct audio_output_info *ai);
bool obs_reset_audio2_open(struct audio_output_info *ai)
{
	return obs_init_audio(ai);
}

/* obs_init_audio() on the info the original builds: "Audio", float planar,
 * the rate and layout given, audio_callback. */
bool obs_reset_audio2_init(uint32_t samples_per_sec, enum speaker_layout speakers);
bool obs_reset_audio2_init(uint32_t samples_per_sec, enum speaker_layout speakers)
{
	struct audio_output_info ai;

	ai.name = "Audio";
	ai.samples_per_sec = samples_per_sec;
	ai.format = AUDIO_FORMAT_FLOAT_PLANAR;
	ai.speakers = speakers;
	ai.input_callback = audio_callback;

	return obs_reset_audio2_open(&ai);
}

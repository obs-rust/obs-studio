#include "libobs/build/libobs.pch.h"

/* For the C reference: the original obs_reset_audio2() calls the static
 * obs_init_audio() and obs_free_audio() of obs.c.h, in the core group. Here,
 * in their own group, they reach those through the private adapters of
 * obs-reset-audio2-fields.c.h. */
bool obs_reset_audio2_open(struct audio_output_info *ai);
void obs_reset_audio2_free(void);

static bool obs_init_audio(struct audio_output_info *ai)
{
	return obs_reset_audio2_open(ai);
}

static void obs_free_audio(void)
{
	obs_reset_audio2_free();
}

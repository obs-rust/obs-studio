#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_get_audio_info() and obs_get_audio_info2()
 * (#5363): the current core, one read of each audio field, and the output's
 * info through the real audio_output_get_info(). The core layout stays in C;
 * the Rust shim checks the core for NULL before it asks for any field. */
const struct obs_core *obs_audio_info_queries_core(void);
const struct obs_core *obs_audio_info_queries_core(void)
{
	return obs;
}

audio_t *obs_audio_info_queries_output(const struct obs_core *core);
audio_t *obs_audio_info_queries_output(const struct obs_core *core)
{
	return core->audio.audio;
}

const struct audio_output_info *obs_audio_info_queries_output_info(const audio_t *audio);
const struct audio_output_info *obs_audio_info_queries_output_info(const audio_t *audio)
{
	return audio_output_get_info(audio);
}

bool obs_audio_info_queries_fixed_buffer(const struct obs_core *core);
bool obs_audio_info_queries_fixed_buffer(const struct obs_core *core)
{
	return core->audio.fixed_buffer;
}

int obs_audio_info_queries_max_buffering_ticks(const struct obs_core *core);
int obs_audio_info_queries_max_buffering_ticks(const struct obs_core *core)
{
	return core->audio.max_buffering_ticks;
}

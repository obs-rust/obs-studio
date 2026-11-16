#include "libobs/build/libobs.pch.h"

/* Private layout and mutex boundary; all decisions live in Rust. */
extern uint32_t obs_output_conversion_flags(const obs_output_t *output);
extern const char *obs_output_conversion_name(const obs_output_t *output);
extern struct video_scale_info *obs_output_conversion_video(obs_output_t *output);
extern struct audio_convert_info *obs_output_conversion_audio(obs_output_t *output);
extern bool *obs_output_conversion_set(obs_output_t *output, bool video);
extern struct darray *obs_output_packet_callbacks_lock(obs_output_t *output);
extern void obs_output_packet_callbacks_unlock(obs_output_t *output);

uint32_t obs_output_conversion_flags(const obs_output_t *output)
{
	return output->info.flags;
}
const char *obs_output_conversion_name(const obs_output_t *output)
{
	return output->context.name;
}
struct video_scale_info *obs_output_conversion_video(obs_output_t *output)
{
	return &output->video_conversion;
}
struct audio_convert_info *obs_output_conversion_audio(obs_output_t *output)
{
	return &output->audio_conversion;
}
bool *obs_output_conversion_set(obs_output_t *output, bool video)
{
	return video ? &output->video_conversion_set : &output->audio_conversion_set;
}
struct darray *obs_output_packet_callbacks_lock(obs_output_t *output)
{
	pthread_mutex_lock(&output->pkt_callbacks_mutex);
	return &output->pkt_callbacks.da;
}
void obs_output_packet_callbacks_unlock(obs_output_t *output)
{
	pthread_mutex_unlock(&output->pkt_callbacks_mutex);
}

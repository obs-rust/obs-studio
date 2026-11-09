#include "libobs/build/libobs.pch.h"

/* Defined in obs-encoder.c.h in this unity group. */
static inline void get_audio_info(const struct obs_encoder *encoder, struct audio_convert_info *info);

static inline void free_audio_buffers(struct obs_encoder *encoder)
{
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		deque_free(&encoder->audio_input_buffer[i]);
		bfree(encoder->audio_output_buffer[i]);
		encoder->audio_output_buffer[i] = NULL;
	}
}

static inline void reset_audio_buffers(struct obs_encoder *encoder)
{
	free_audio_buffers(encoder);

	for (size_t i = 0; i < encoder->planes; i++)
		encoder->audio_output_buffer[i] = bmalloc(encoder->framesize_bytes);
}

static void intitialize_audio_encoder(struct obs_encoder *encoder)
{
	struct audio_convert_info info = {0};
	get_audio_info(encoder, &info);

	encoder->samplerate = info.samples_per_sec;
	encoder->planes = get_audio_planes(info.format, info.speakers);
	encoder->blocksize = get_audio_size(info.format, info.speakers, 1);
	encoder->framesize = encoder->info.get_frame_size(encoder->context.data);

	encoder->framesize_bytes = encoder->blocksize * encoder->framesize;
	reset_audio_buffers(encoder);
}

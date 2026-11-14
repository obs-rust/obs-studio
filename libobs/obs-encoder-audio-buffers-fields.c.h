#include "libobs/build/libobs.pch.h"

static inline void get_audio_info(const struct obs_encoder *encoder, struct audio_convert_info *info);

static struct obs_encoder_audio_buffers_view encoder_audio_buffers_view(struct obs_encoder *encoder)
{
	return (struct obs_encoder_audio_buffers_view){encoder->audio_input_buffer, encoder->audio_output_buffer,
						       &encoder->samplerate,        &encoder->planes,
						       &encoder->blocksize,         &encoder->framesize,
						       &encoder->framesize_bytes,   encoder->info.get_frame_size,
						       encoder->context.data};
}

static inline void free_audio_buffers(struct obs_encoder *encoder)
{
	struct obs_encoder_audio_buffers_view view = encoder_audio_buffers_view(encoder);
	obs_encoder_audio_buffers_free(&view);
}

static inline void reset_audio_buffers(struct obs_encoder *encoder)
{
	struct obs_encoder_audio_buffers_view view = encoder_audio_buffers_view(encoder);
	obs_encoder_audio_buffers_reset(&view);
}

static void intitialize_audio_encoder(struct obs_encoder *encoder)
{
	struct audio_convert_info info = {0};
	get_audio_info(encoder, &info);
	struct obs_encoder_audio_buffers_view view = encoder_audio_buffers_view(encoder);
	obs_encoder_audio_buffers_initialize(&view, info.samples_per_sec, info.format, info.speakers);
}

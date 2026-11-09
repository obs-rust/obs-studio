/* Exact original static lifecycle on the real private encoder layout. */
#include "obs-internal.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

struct encoder_buffers_fixture {
	struct obs_encoder encoder;
	struct audio_convert_info negotiated;
	size_t frames;
	size_t info_calls;
	size_t frame_calls;
	size_t observed_rate;
	size_t observed_planes;
	size_t observed_block;
	bool callback_mutates;
};
static void get_audio_info(const struct obs_encoder *encoder, struct audio_convert_info *info)
{
	struct encoder_buffers_fixture *f = encoder->context.data;
	if (encoder != &f->encoder || info->format != AUDIO_FORMAT_UNKNOWN || info->samples_per_sec != 0 ||
	    info->speakers != SPEAKERS_UNKNOWN)
		abort();
	*info = f->negotiated;
	f->info_calls++;
}
#define free_audio_buffers original_free_audio_buffers
#define reset_audio_buffers original_reset_audio_buffers
#define intitialize_audio_encoder original_intitialize_audio_encoder
#include "encoder_audio_buffers_original.inc"
#undef free_audio_buffers
#undef reset_audio_buffers
#undef intitialize_audio_encoder
#include "encoder_audio_buffers_fields.inc"

static size_t encoder_buffers_frame_size(void *context)
{
	struct encoder_buffers_fixture *f = context;
	if (f->info_calls != f->frame_calls + 1)
		abort();
	f->frame_calls++;
	f->observed_rate = f->encoder.samplerate;
	f->observed_planes = f->encoder.planes;
	f->observed_block = f->encoder.blocksize;
	if (f->callback_mutates) {
		f->encoder.blocksize = 3;
		f->encoder.planes = 2;
	}
	return f->frames;
}
void *oracle_encoder_buffers_new(uint32_t rate, int format, int speakers, size_t frames, bool mutate);
void *oracle_encoder_buffers_new(uint32_t rate, int format, int speakers, size_t frames, bool mutate)
{
	struct encoder_buffers_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		abort();
	f->negotiated = (struct audio_convert_info){.samples_per_sec = rate,
						    .format = (enum audio_format)format,
						    .speakers = (enum speaker_layout)speakers};
	f->frames = frames;
	f->callback_mutates = mutate;
	f->encoder.context.data = f;
	f->encoder.info.get_frame_size = encoder_buffers_frame_size;
	f->encoder.planes = 1;
	f->encoder.framesize_bytes = 37;
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		const uint8_t input[] = {1, 2, 3, 4, 5};
		deque_push_back(&f->encoder.audio_input_buffer[i], input, sizeof(input));
		f->encoder.audio_output_buffer[i] = bmalloc(19 + i);
	}
	return f;
}
void oracle_encoder_buffers_view(void *fixture, struct obs_encoder_audio_buffers_view *view);
void oracle_encoder_buffers_view(void *fixture, struct obs_encoder_audio_buffers_view *view)
{
	struct encoder_buffers_fixture *f = fixture;
	struct obs_encoder *e = &f->encoder;
	*view = (struct obs_encoder_audio_buffers_view){
		e->audio_input_buffer, e->audio_output_buffer, &e->samplerate,         &e->planes,     &e->blocksize,
		&e->framesize,         &e->framesize_bytes,    e->info.get_frame_size, e->context.data};
}
void oracle_encoder_buffers_run(void *fixture, int operation);
void oracle_encoder_buffers_run(void *fixture, int operation)
{
	struct encoder_buffers_fixture *f = fixture;
	if (operation == 0)
		original_free_audio_buffers(&f->encoder);
	else if (operation == 1)
		original_reset_audio_buffers(&f->encoder);
	else
		original_intitialize_audio_encoder(&f->encoder);
}
void oracle_encoder_buffers_rust_run(void *fixture, int operation);
void oracle_encoder_buffers_rust_run(void *fixture, int operation)
{
	struct encoder_buffers_fixture *f = fixture;
	if (operation == 0)
		free_audio_buffers(&f->encoder);
	else if (operation == 1)
		reset_audio_buffers(&f->encoder);
	else
		intitialize_audio_encoder(&f->encoder);
}
/* Scalar observable state, including every deque member and output presence.
 * Touch all promised bytes; the old allocations' contents are unspecified. */
void oracle_encoder_buffers_snapshot(void *fixture, size_t *out);
void oracle_encoder_buffers_snapshot(void *fixture, size_t *out)
{
	struct encoder_buffers_fixture *f = fixture;
	struct obs_encoder *e = &f->encoder;
	*out++ = e->samplerate;
	*out++ = e->planes;
	*out++ = e->blocksize;
	*out++ = e->framesize;
	*out++ = e->framesize_bytes;
	*out++ = f->frame_calls;
	*out++ = f->info_calls;
	*out++ = f->observed_rate;
	*out++ = f->observed_planes;
	*out++ = f->observed_block;
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		struct deque *q = &e->audio_input_buffer[i];
		*out++ = q->data != NULL;
		*out++ = q->size;
		*out++ = q->start_pos;
		*out++ = q->end_pos;
		*out++ = q->capacity;
		*out++ = e->audio_output_buffer[i] != NULL;
		if (i < e->planes && e->audio_output_buffer[i])
			memset(e->audio_output_buffer[i], 0x5a, e->framesize_bytes);
	}
}
void oracle_encoder_buffers_delete(void *fixture);
void oracle_encoder_buffers_delete(void *fixture)
{
	struct encoder_buffers_fixture *f = fixture;
	original_free_audio_buffers(&f->encoder);
	free(f);
}
void oracle_encoder_buffers_layout(size_t *out);
void oracle_encoder_buffers_layout(size_t *out)
{
	*out++ = sizeof(struct obs_encoder_audio_buffers_view);
	*out++ = _Alignof(struct obs_encoder_audio_buffers_view);
	*out++ = offsetof(struct obs_encoder_audio_buffers_view, input);
	*out++ = offsetof(struct obs_encoder_audio_buffers_view, output);
	*out++ = offsetof(struct obs_encoder_audio_buffers_view, rate);
	*out++ = offsetof(struct obs_encoder_audio_buffers_view, planes);
	*out++ = offsetof(struct obs_encoder_audio_buffers_view, blocksize);
	*out++ = offsetof(struct obs_encoder_audio_buffers_view, frames);
	*out++ = offsetof(struct obs_encoder_audio_buffers_view, bytes);
	*out++ = offsetof(struct obs_encoder_audio_buffers_view, frame_size);
	*out++ = offsetof(struct obs_encoder_audio_buffers_view, context);
	*out++ = MAX_AV_PLANES;
}

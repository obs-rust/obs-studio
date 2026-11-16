/* Pins the static C buffer lifecycle, not the future Rust replacement.
 * get_audio_info is the dependency seam: supply the final negotiated format. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static struct audio_convert_info negotiated;
static size_t info_calls, frame_calls;
static int frame_count;
static int context_token;

static void get_audio_info(const struct obs_encoder *encoder, struct audio_convert_info *info)
{
	assert_ptr_equal(encoder->context.data, &context_token);
	assert_int_equal(info->format, 0);
	assert_int_equal(info->samples_per_sec, 0);
	assert_int_equal(info->speakers, 0);
	*info = negotiated;
	info_calls++;
}

static size_t frame_size(void *data)
{
	assert_ptr_equal(data, &context_token);
	assert_int_equal(info_calls, 1);
	frame_calls++;
	return frame_count;
}

#include "encoder-audio-buffers-under-test.h"

static void populate(struct obs_encoder *encoder)
{
	const unsigned char bytes[] = {1, 2, 3, 4, 5};
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		deque_push_back(&encoder->audio_input_buffer[i], bytes, sizeof(bytes));
		encoder->audio_output_buffer[i] = bmalloc(19 + i);
	}
}

static void assert_inputs_empty(const struct obs_encoder *encoder)
{
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		const struct deque empty = {0};
		assert_memory_equal(&encoder->audio_input_buffer[i], &empty, sizeof(empty));
	}
}

static void cleanup_all_planes(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	long baseline = bnum_allocs();
	/* Cleanup uses MAX_AV_PLANES, including stale planes above encoder.planes. */
	encoder.planes = 1;
	encoder.framesize_bytes = 37;
	populate(&encoder);
	free_audio_buffers(&encoder);
	assert_inputs_empty(&encoder);
	for (size_t i = 0; i < MAX_AV_PLANES; i++)
		assert_null(encoder.audio_output_buffer[i]);
	assert_int_equal(encoder.planes, 1);
	assert_int_equal(encoder.framesize_bytes, 37);
	assert_int_equal(bnum_allocs(), baseline);
	free_audio_buffers(&encoder);
	assert_int_equal(bnum_allocs(), baseline);
}

static void reset_active_planes(void **state)
{
	(void)state;
	const size_t plane_counts[] = {0, 1, 2, MAX_AV_PLANES};
	for (size_t n = 0; n < sizeof(plane_counts) / sizeof(plane_counts[0]); n++) {
		struct obs_encoder encoder = {0};
		long baseline = bnum_allocs();
		populate(&encoder);
		encoder.planes = plane_counts[n];
		encoder.framesize_bytes = 37;
		for (size_t repeat = 0; repeat < 2; repeat++) {
			reset_audio_buffers(&encoder);
			assert_inputs_empty(&encoder);
			for (size_t i = 0; i < MAX_AV_PLANES; i++) {
				if (i < encoder.planes) {
					assert_non_null(encoder.audio_output_buffer[i]);
					/* Touch every promised byte so sanitizers check the allocation size. */
					memset(encoder.audio_output_buffer[i], 0x5a, encoder.framesize_bytes);
				} else {
					assert_null(encoder.audio_output_buffer[i]);
				}
			}
			assert_int_equal(bnum_allocs(), baseline + (long)encoder.planes);
		}
		free_audio_buffers(&encoder);
		assert_int_equal(bnum_allocs(), baseline);
	}
}

static void initialize_geometry(void **state)
{
	(void)state;
	const struct {
		enum audio_format format;
		enum speaker_layout speakers;
		uint32_t rate;
		int frames;
		size_t planes, blocksize;
	} cases[] = {{AUDIO_FORMAT_FLOAT_PLANAR, SPEAKERS_STEREO, 48000, 1024, 2, 4},
		     {AUDIO_FORMAT_16BIT, SPEAKERS_STEREO, 44100, 960, 1, 4},
		     {AUDIO_FORMAT_16BIT_PLANAR, SPEAKERS_5POINT1, 32000, 17, 6, 2},
		     {AUDIO_FORMAT_U8BIT, SPEAKERS_MONO, 22050, 1, 1, 1}};
	for (size_t n = 0; n < sizeof(cases) / sizeof(cases[0]); n++) {
		struct obs_encoder encoder = {0};
		long baseline = bnum_allocs();
		populate(&encoder);
		encoder.context.data = &context_token;
		encoder.info.get_frame_size = frame_size;
		negotiated = (struct audio_convert_info){cases[n].rate, cases[n].format, cases[n].speakers};
		frame_count = cases[n].frames;
		info_calls = frame_calls = 0;
		intitialize_audio_encoder(&encoder);
		assert_int_equal(info_calls, 1);
		assert_int_equal(frame_calls, 1);
		assert_int_equal(encoder.samplerate, cases[n].rate);
		assert_int_equal(encoder.planes, cases[n].planes);
		assert_int_equal(encoder.blocksize, cases[n].blocksize);
		assert_int_equal(encoder.framesize, cases[n].frames);
		assert_int_equal(encoder.framesize_bytes, cases[n].blocksize * cases[n].frames);
		assert_inputs_empty(&encoder);
		for (size_t i = 0; i < MAX_AV_PLANES; i++) {
			if (i < encoder.planes) {
				assert_non_null(encoder.audio_output_buffer[i]);
				memset(encoder.audio_output_buffer[i], 0xa5, encoder.framesize_bytes);
			} else {
				assert_null(encoder.audio_output_buffer[i]);
			}
		}
		assert_int_equal(bnum_allocs(), baseline + (long)encoder.planes);
		free_audio_buffers(&encoder);
		assert_int_equal(bnum_allocs(), baseline);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(cleanup_all_planes), cmocka_unit_test(reset_active_planes),
					   cmocka_unit_test(initialize_geometry)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

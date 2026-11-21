/* External Rust boundary: the unchanged characterization assertions and real C
 * registration/lookup, with private C video-io names beside the Rust archive. */
#define ENCODER_ALIGN_SOURCE "test/cmocka/encoder-align-frame-rate-divisor-boundary.c"
#define video_output_open align_c_video_output_open
#define video_output_close align_c_video_output_close
#define video_output_connect align_c_video_output_connect
#define video_output_connect2 align_c_video_output_connect2
#define video_output_disconnect align_c_video_output_disconnect
#define video_output_disconnect2 align_c_video_output_disconnect2
#define video_output_align_frame_rate_divisor align_c_video_output_align_frame_rate_divisor
#define video_output_active align_c_video_output_active
#define video_output_get_info align_c_video_output_get_info
#define video_output_lock_frame align_c_video_output_lock_frame
#define video_output_unlock_frame align_c_video_output_unlock_frame
#define video_output_get_frame_time align_c_video_output_get_frame_time
#define video_output_stop align_c_video_output_stop
#define video_output_stopped align_c_video_output_stopped
#define video_output_get_format align_c_video_output_get_format
#define video_output_get_width align_c_video_output_get_width
#define video_output_get_height align_c_video_output_get_height
#define video_output_get_frame_rate align_c_video_output_get_frame_rate
#define video_output_get_skipped_frames align_c_video_output_get_skipped_frames
#define video_output_get_total_frames align_c_video_output_get_total_frames
#define video_output_inc_texture_encoders align_c_video_output_inc_texture_encoders
#define video_output_dec_texture_encoders align_c_video_output_dec_texture_encoders
#define video_output_inc_texture_frames align_c_video_output_inc_texture_frames
#define video_output_inc_texture_skipped_frames align_c_video_output_inc_texture_skipped_frames
#define video_output_create_with_frame_rate_divisor align_c_video_output_create_with_frame_rate_divisor
#define video_output_free_frame_rate_divisor align_c_video_output_free_frame_rate_divisor
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "libobs/media-io/video-io.c.h"
#undef video_output_align_frame_rate_divisor

static unsigned align_calls;
static unsigned callback_calls;
static video_t *aligned_media;
static uint64_t aligned_timestamp;
static void (*aligned_callback)(void *, struct video_data *);
static void *aligned_param;
static bool matched;

static void receive_video(void *param, struct video_data *frame)
{
	(void)param;
	(void)frame;
	callback_calls++;
}

static void other_callback(void *param, struct video_data *frame)
{
	(void)param;
	(void)frame;
	/* Keep a distinct body: identical-code folding must not merge the decoy
	 * address with receive_video in an optimized Windows build. */
	callback_calls += 2;
}

static bool record_alignment(video_t *media, uint64_t ts, void (*callback)(void *, struct video_data *), void *param)
{
	align_calls++;
	aligned_media = media;
	aligned_timestamp = ts;
	aligned_callback = callback;
	aligned_param = param;
	matched = align_c_video_output_align_frame_rate_divisor(media, ts, callback, param);
	return matched;
}

#ifndef ENCODER_ALIGN_SOURCE
#define ENCODER_ALIGN_SOURCE "libobs/obs-encoder-align-frame-rate-divisor.c.h"
#endif
#define video_output_align_frame_rate_divisor record_alignment
#include ENCODER_ALIGN_SOURCE
#undef video_output_align_frame_rate_divisor

static void no_call_guards(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	struct video_output media = {0};
	/* The media mutex is intentionally uninitialized: every case must return
	 * before entering video-io. NULL media wins even for the largest divisor. */
	align_calls = 0;
	callback_calls = 0;
	obs_encoder_align_frame_rate_divisor(NULL, UINT64_MAX);
	encoder.frame_rate_divisor = UINT32_MAX;
	obs_encoder_align_frame_rate_divisor(&encoder, UINT64_MAX);
	encoder.media = &media;
	for (uint32_t divisor = 0; divisor < 2; divisor++) {
		encoder.frame_rate_divisor = divisor;
		struct obs_encoder before;
		memcpy(&before, &encoder, sizeof(before));
		obs_encoder_align_frame_rate_divisor(&encoder, UINT64_MAX);
		assert_memory_equal(&encoder, &before, sizeof(encoder));
	}
	assert_int_equal(align_calls, 0);
	assert_int_equal(callback_calls, 0);
}

static void registered_identity_and_forwarding(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	struct obs_encoder other = {0};
	struct video_output root = {0};
	struct video_output child = {0};
	root.info.format = VIDEO_FORMAT_RGBA;
	root.info.width = root.info.height = 16;
	assert_int_equal(pthread_mutex_init(&root.input_mutex, NULL), 0);
	child.parent = &root;
	/* The first two registrations differ in only one identity component.
	 * Matching only the callback or only the encoder would select the wrong
	 * input; using a replacement callback would select none. */
	assert_true(video_output_connect2(&root, NULL, 3, receive_video, &other));
	assert_true(video_output_connect2(&root, NULL, 3, other_callback, &encoder));
	assert_true(video_output_connect2(&root, NULL, 3, receive_video, &encoder));
	assert_int_equal(root.inputs.num, 3);
	assert_true(root.inputs.array[2].callback == receive_video);
	assert_ptr_equal(root.inputs.array[2].param, &encoder);

	const uint32_t divisors[] = {2, 3, UINT32_MAX};
	const uint64_t timestamps[] = {0, 1, UINT64_C(0x123456789abcdef0), UINT64_MAX};
	for (size_t view = 0; view < 2; view++) {
		encoder.media = view ? &child : &root;
		for (size_t d = 0; d < sizeof(divisors) / sizeof(divisors[0]); d++) {
			encoder.frame_rate_divisor = divisors[d];
			for (size_t t = 0; t < sizeof(timestamps) / sizeof(timestamps[0]); t++) {
				for (size_t i = 0; i < root.inputs.num; i++) {
					root.inputs.array[i].frame_rate_divisor_counter = 1;
					root.inputs.array[i].start_timestamp = 99;
				}
				struct obs_encoder before;
				memcpy(&before, &encoder, sizeof(before));
				long allocations = bnum_allocs();
				align_calls = 0;
				callback_calls = 0;
				matched = false;
				obs_encoder_align_frame_rate_divisor(&encoder, timestamps[t]);
				assert_int_equal(align_calls, 1);
				assert_ptr_equal(aligned_media, encoder.media);
				assert_int_equal(aligned_timestamp, timestamps[t]);
				assert_true(aligned_callback == root.inputs.array[2].callback);
				assert_ptr_equal(aligned_param, root.inputs.array[2].param);
				assert_true(matched);
				assert_int_equal(root.inputs.array[2].frame_rate_divisor_counter, 0);
				assert_int_equal(root.inputs.array[2].start_timestamp, timestamps[t]);
				for (size_t i = 0; i < 2; i++) {
					assert_int_equal(root.inputs.array[i].frame_rate_divisor_counter, 1);
					assert_int_equal(root.inputs.array[i].start_timestamp, 99);
				}
				assert_memory_equal(&encoder, &before, sizeof(encoder));
				assert_int_equal(bnum_allocs(), allocations);
				assert_int_equal(callback_calls, 0);
			}
		}
	}
	assert_true(video_output_disconnect2(&root, receive_video, &encoder));
	assert_true(video_output_disconnect2(&root, other_callback, &encoder));
	assert_true(video_output_disconnect2(&root, receive_video, &other));
	da_free(root.inputs);
	assert_int_equal(pthread_mutex_destroy(&root.input_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(no_call_guards),
					   cmocka_unit_test(registered_identity_and_forwarding)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

#undef video_output_open
#undef video_output_close
#undef video_output_connect
#undef video_output_connect2
#undef video_output_disconnect
#undef video_output_disconnect2
#undef video_output_active
#undef video_output_get_info
#undef video_output_lock_frame
#undef video_output_unlock_frame
#undef video_output_get_frame_time
#undef video_output_stop
#undef video_output_stopped
#undef video_output_get_format
#undef video_output_get_width
#undef video_output_get_height
#undef video_output_get_frame_rate
#undef video_output_get_skipped_frames
#undef video_output_get_total_frames
#undef video_output_inc_texture_encoders
#undef video_output_dec_texture_encoders
#undef video_output_inc_texture_frames
#undef video_output_inc_texture_skipped_frames
#undef video_output_create_with_frame_rate_divisor
#undef video_output_free_frame_rate_divisor

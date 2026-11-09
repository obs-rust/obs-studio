/* Direct characterization of obs_output_caption, before its Rust swap.
 * The output needs only caption_tracks; no encoder or running OBS core. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"
#include "util/bmem.h"

static void sparse_tracks_copy_and_append(void **state)
{
	(void)state;
	const unsigned masks[] = {0, 1, 1U << (MAX_OUTPUT_VIDEO_ENCODERS - 1), 21,
				  (1U << MAX_OUTPUT_VIDEO_ENCODERS) - 1};
	for (size_t m = 0; m < sizeof(masks) / sizeof(masks[0]); m++) {
		long before = bnum_allocs();
		obs_output_t *output = bzalloc(sizeof(*output));
		for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
			if (!(masks[m] & (1U << i)))
				continue;
			struct caption_track_data *track = bzalloc(sizeof(*track));
			assert_int_equal(pthread_mutex_init(&track->caption_mutex, NULL), 0);
			deque_init(&track->caption_data);
			const uint8_t prefix[] = {9, 8, 7};
			deque_push_back(&track->caption_data, prefix, sizeof(prefix));
			output->caption_tracks[i] = track;
		}
		struct obs_source_cea_708 empty = {0};
		obs_output_caption(output, &empty);
		uint8_t bytes[] = {4, 193, 194, 4, 67, 196};
		struct obs_source_cea_708 captions = {.data = bytes, .packets = 2};
		obs_output_caption(output, &captions);
		memset(bytes, 0xff, sizeof(bytes));
		const uint8_t tail[] = {5, 6, 7};
		captions.data = tail;
		captions.packets = 1;
		obs_output_caption(output, &captions);
		obs_output_caption(output, &empty);
		const uint8_t expected[] = {9, 8, 7, 4, 193, 194, 4, 67, 196, 5, 6, 7};
		for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
			struct caption_track_data *track = output->caption_tracks[i];
			if (!track)
				continue;
			assert_int_equal(track->caption_data.size, sizeof(expected));
			uint8_t actual[sizeof(expected)];
			deque_pop_front(&track->caption_data, actual, sizeof(actual));
			assert_memory_equal(actual, expected, sizeof(expected));
			assert_int_equal(track->caption_data.size, 0);
			deque_free(&track->caption_data);
			assert_int_equal(pthread_mutex_destroy(&track->caption_mutex), 0);
			bfree(track);
		}
		bfree(output);
		assert_int_equal(bnum_allocs(), before);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(sparse_tracks_copy_and_append)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

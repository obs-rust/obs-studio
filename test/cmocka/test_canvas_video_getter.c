/* Characterize the live borrowed video getter; no lock or reference is acquired. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void borrowed_video(void **state)
{
	(void)state;
	struct obs_canvas canvas = {0};
	struct obs_core_video_mix mix = {0};
	struct obs_canvas before;
	struct obs_core_video_mix mix_before;
	void *video_tokens[2] = {malloc(1), malloc(1)};
	assert_non_null(video_tokens[0]);
	assert_non_null(video_tokens[1]);
	for (unsigned mode = 0; mode < 4; mode++) {
		canvas.mix = mode ? &mix : NULL;
		mix.video = mode >= 2 ? (video_t *)video_tokens[mode - 2] : NULL;
		memcpy(&before, &canvas, sizeof(canvas));
		memcpy(&mix_before, &mix, sizeof(mix));
		video_t *expected = mode >= 2 ? mix.video : NULL;
		assert_ptr_equal(obs_canvas_get_video(&canvas), expected);
		assert_ptr_equal(obs_canvas_get_video(&canvas), expected);
		assert_memory_equal(&canvas, &before, sizeof(canvas));
		assert_memory_equal(&mix, &mix_before, sizeof(mix));
	}
	free(video_tokens[0]);
	free(video_tokens[1]);
	/* Characterized, not endorsed: canvas must be non-NULL. A missing mix or
	 * NULL video returns NULL. Non-NULL outputs are borrowed, never touched. */
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(borrowed_video)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

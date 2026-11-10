/* C-source characterization only: like test_video_stop_requested, this records
 * a private helper without starting graphics. A later Rust swap must additionally
 * exercise its live callers; compiling this source alone cannot test that swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

static struct obs_core_video_mix unmap_mix;
/* Opaque identity tokens are never dereferenced (also portable to MSVC C). */
static long double unmap_tokens[NUM_CHANNELS];
static unsigned unmap_mask;
static int unmap_next;
static int unmap_calls;

static void unmap_record(gs_stagesurf_t *surface)
{
	while (unmap_next < NUM_CHANNELS && !(unmap_mask & (1U << unmap_next)))
		unmap_next++;
	assert_true(unmap_next < NUM_CHANNELS);
	assert_ptr_equal(surface, &unmap_tokens[unmap_next]);
	/* Earlier slots have reset; the current and later slots retain their
	 * original identity until their own unmap callback has returned. */
	for (int c = 0; c < NUM_CHANNELS; c++) {
		gs_stagesurf_t *expected =
			c >= unmap_next && (unmap_mask & (1U << c)) ? (gs_stagesurf_t *)&unmap_tokens[c] : NULL;
		assert_ptr_equal(unmap_mix.mapped_surfaces[c], expected);
	}
	unmap_next++;
	unmap_calls++;
}

#define gs_stagesurface_unmap unmap_record
#include UNMAP_SOURCE
#undef gs_stagesurface_unmap

static void unmap_all_channel_masks(void **state)
{
	(void)state;
	for (unsigned mask = 0; mask < (1U << NUM_CHANNELS); mask++) {
		unmap_mask = mask;
		unmap_next = 0;
		unmap_calls = 0;
		int expected_calls = 0;
		for (int c = 0; c < NUM_CHANNELS; c++) {
			bool mapped = (mask & (1U << c)) != 0;
			unmap_mix.mapped_surfaces[c] = mapped ? (gs_stagesurf_t *)&unmap_tokens[c] : NULL;
			expected_calls += mapped;
		}
		unmap_last_surface(&unmap_mix);
		assert_int_equal(unmap_calls, expected_calls);
		for (int c = 0; c < NUM_CHANNELS; c++)
			assert_null(unmap_mix.mapped_surfaces[c]);
		/* Empty and repeated cleanup make no graphics calls. */
		unmap_last_surface(&unmap_mix);
		assert_int_equal(unmap_calls, expected_calls);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(unmap_all_channel_masks)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

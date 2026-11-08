/* Characterize borrowed getters through the actual libobs exports. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void borrowed_fields(void **state)
{
	(void)state;
	struct obs_canvas canvas = {0};
	struct obs_canvas before;
	char name[] = "canvas name";
	char uuid[] = "12345678-1234-1234-1234-123456789abc";
	/* MSVC's C headers lack max_align_t. This borrowed token is never dereferenced. */
	_Alignas(long double) unsigned char signal_token = 0;
	struct obs_core_video_mix mix = {0};
	for (unsigned mask = 0; mask < 16; mask++) {
		canvas.context.name = mask & 1 ? name : NULL;
		canvas.context.uuid = mask & 2 ? uuid : NULL;
		canvas.context.signals = mask & 4 ? (signal_handler_t *)&signal_token : NULL;
		canvas.mix = mask & 8 ? &mix : NULL;
		memcpy(&before, &canvas, sizeof(canvas));
		assert_ptr_equal(obs_canvas_get_name(&canvas), canvas.context.name);
		assert_ptr_equal(obs_canvas_get_uuid(&canvas), canvas.context.uuid);
		assert_ptr_equal(obs_canvas_get_signal_handler(&canvas), canvas.context.signals);
		assert_int_equal(obs_canvas_has_video(&canvas), (mask & 8) != 0);
		assert_memory_equal(&canvas, &before, sizeof(canvas));
	}
	/* Characterized, not endorsed: getters require a non-NULL canvas.
	 * Borrowed fields may be NULL, and a present mix need not have video. */
	assert_null(mix.video);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(borrowed_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

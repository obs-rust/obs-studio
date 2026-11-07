/* Characterize the actual public export, including stored unknown flag bits. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void stored_flags(void **state)
{
	(void)state;
	struct obs_canvas canvas = {0};
	struct obs_canvas before;
	const uint32_t values[] = {0,         1, 2, 4, 8, 16, 32, 64, 128, UINT32_C(0x80000000), UINT32_C(0xa5a5a5a5),
				   UINT32_MAX};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		canvas.flags = values[i];
		memcpy(&before, &canvas, sizeof(canvas));
		assert_int_equal(obs_canvas_get_flags(&canvas), values[i]);
		assert_memory_equal(&canvas, &before, sizeof(canvas));
	}
	/* Characterized, not endorsed: a non-NULL canvas is required and unknown
	 * stored bits are returned without applying the creation-time public mask. */
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(stored_flags)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

/* Actual-library characterization: readers must not mutate private state. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[128];

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static void null_defaults(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	enum obs_deinterlace_mode mode = obs_source_get_deinterlace_mode(NULL);
	unsigned mode_count = log_count;
	int mode_level = log_level;
	char mode_message[128];
	memcpy(mode_message, log_message, sizeof(mode_message));
	enum obs_deinterlace_field_order order = obs_source_get_deinterlace_field_order(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(mode, OBS_DEINTERLACE_MODE_DISABLE);
	assert_int_equal(order, OBS_DEINTERLACE_FIELD_ORDER_TOP);
	assert_int_equal(mode_count, 1);
	assert_int_equal(log_count, 2);
	assert_int_equal(mode_level, LOG_DEBUG);
	assert_int_equal(log_level, LOG_DEBUG);
	/* Characterized, not endorsed: both readers name their setter. */
	assert_string_equal(mode_message, "obs_source_set_deinterlace_mode: Null 'source' parameter");
	assert_string_equal(log_message, "obs_source_set_deinterlace_field_order: Null 'source' parameter");
}

static void independent_fields(void **state)
{
	(void)state;
	struct obs_source source = {0};
	for (int mode = OBS_DEINTERLACE_MODE_DISABLE; mode <= OBS_DEINTERLACE_MODE_YADIF_2X; mode++) {
		for (int top = 0; top < 2; top++) {
			source.deinterlace_mode = (enum obs_deinterlace_mode)mode;
			source.deinterlace_top_first = top != 0;
			source.async_width = 17;
			source.async_height = 29;
			assert_int_equal(obs_source_get_deinterlace_mode(&source), mode);
			assert_int_equal(obs_source_get_deinterlace_field_order(&source),
					 top ? OBS_DEINTERLACE_FIELD_ORDER_TOP : OBS_DEINTERLACE_FIELD_ORDER_BOTTOM);
			assert_int_equal(source.deinterlace_mode, mode);
			assert_int_equal(source.deinterlace_top_first, top != 0);
			assert_int_equal(source.async_width, 17);
			assert_int_equal(source.async_height, 29);
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults), cmocka_unit_test(independent_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

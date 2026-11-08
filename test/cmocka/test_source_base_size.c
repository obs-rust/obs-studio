/* Characterization of the public base-size queries in the linked libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

static uint32_t base_width(void *data)
{
	return *(uint32_t *)data;
}

static uint32_t base_height(void *data)
{
	return *(uint32_t *)data + 1;
}

static void source_base_size(void **state)
{
	(void)state;
	assert_int_equal(obs_source_get_base_width(NULL), 0);
	assert_int_equal(obs_source_get_base_height(NULL), 0);
	uint32_t size = 123;
	struct obs_source source = {0};
	source.context.data = &size;
	assert_int_equal(obs_source_get_base_width(&source), 0);
	assert_int_equal(obs_source_get_base_height(&source), 0);
	source.info.get_width = base_width;
	source.info.get_height = base_height;
	assert_int_equal(obs_source_get_base_width(&source), 123);
	assert_int_equal(obs_source_get_base_height(&source), 124);
	size = UINT32_MAX;
	assert_int_equal(obs_source_get_base_width(&source), UINT32_MAX);
	assert_int_equal(obs_source_get_base_height(&source), 0);
	source.info.type = OBS_SOURCE_TYPE_TRANSITION;
	source.enabled = true;
	source.transition_actual_cx = 640;
	source.transition_actual_cy = 480;
	assert_int_equal(obs_source_get_base_width(&source), 640);
	assert_int_equal(obs_source_get_base_height(&source), 480);
	source.info.type = OBS_SOURCE_TYPE_INPUT;
	source.context.data = NULL;
	assert_int_equal(obs_source_get_base_width(&source), 0);
	assert_int_equal(obs_source_get_base_height(&source), 0);
	source.context.data = &size;
	source.info.get_width = NULL;
	source.info.get_height = NULL;
	source.async_active = true;
	source.async_width = 1920;
	source.async_height = 1080;
	for (int angle = -360; angle <= 360; angle += 90) {
		source.async_rotation = angle;
		bool swap = angle % 180 != 0;
		assert_int_equal(obs_source_get_base_width(&source), swap ? 1080 : 1920);
		assert_int_equal(obs_source_get_base_height(&source), swap ? 1920 : 1080);
	}
	struct obs_source filter = {0};
	filter.context.data = &size;
	filter.info.type = OBS_SOURCE_TYPE_FILTER;
	filter.filter_parent = &source;
	assert_int_equal(obs_source_get_base_width(&filter), 0);
	assert_int_equal(obs_source_get_base_height(&filter), 0);
	filter.filter_target = &source;
	source.async_rotation = 0;
	assert_int_equal(obs_source_get_base_width(&filter), 1920);
	assert_int_equal(obs_source_get_base_height(&filter), 1080);
	source.async_active = false;
	assert_int_equal(obs_source_get_base_width(&filter), 0);
	assert_int_equal(obs_source_get_base_height(&filter), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(source_base_size)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

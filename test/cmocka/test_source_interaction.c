/* Pin the existing public interaction dispatch before its Rust replacement. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs.h>

static unsigned calls[5];
static void *expected_data;
static const struct obs_mouse_event *expected_mouse;
static const struct obs_key_event *expected_key;
static bool expected_bool;
static int32_t expected_type;
static uint32_t expected_count;
static int expected_x, expected_y;

static const char *source_name(void *data)
{
	(void)data;
	return "interaction characterization";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(1);
}

static void source_destroy(void *data)
{
	bfree(data);
}

static void mouse_click(void *data, const struct obs_mouse_event *event, int32_t type, bool up, uint32_t count)
{
	assert_ptr_equal(data, expected_data);
	assert_ptr_equal(event, expected_mouse);
	assert_int_equal(type, expected_type);
	assert_int_equal(up, expected_bool);
	assert_int_equal(count, expected_count);
	calls[0]++;
}

static void mouse_move(void *data, const struct obs_mouse_event *event, bool leave)
{
	assert_ptr_equal(data, expected_data);
	assert_ptr_equal(event, expected_mouse);
	assert_int_equal(leave, expected_bool);
	calls[1]++;
}

static void mouse_wheel(void *data, const struct obs_mouse_event *event, int x, int y)
{
	assert_ptr_equal(data, expected_data);
	assert_ptr_equal(event, expected_mouse);
	assert_int_equal(x, expected_x);
	assert_int_equal(y, expected_y);
	calls[2]++;
}

static void focus(void *data, bool value)
{
	assert_ptr_equal(data, expected_data);
	assert_int_equal(value, expected_bool);
	calls[3]++;
}

static void key_click(void *data, const struct obs_key_event *event, bool up)
{
	assert_ptr_equal(data, expected_data);
	assert_ptr_equal(event, expected_key);
	assert_int_equal(up, expected_bool);
	calls[4]++;
}

static void dispatch(obs_source_t *source)
{
	obs_source_send_mouse_click(source, expected_mouse, expected_type, expected_bool, expected_count);
	obs_source_send_mouse_move(source, expected_mouse, expected_bool);
	obs_source_send_mouse_wheel(source, expected_mouse, expected_x, expected_y);
	obs_source_send_focus(source, expected_bool);
	obs_source_send_key_click(source, expected_key, expected_bool);
}

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct obs_source_info info = {
		.id = "interaction_enabled",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_INTERACTION,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
		.mouse_click = mouse_click,
		.mouse_move = mouse_move,
		.mouse_wheel = mouse_wheel,
		.focus = focus,
		.key_click = key_click,
	};
	obs_register_source(&info);
	info.id = "interaction_disabled";
	info.output_flags = 0;
	obs_register_source(&info);
	info.id = "interaction_missing_callbacks";
	info.output_flags = OBS_SOURCE_INTERACTION;
	info.mouse_click = NULL;
	info.mouse_move = NULL;
	info.mouse_wheel = NULL;
	info.focus = NULL;
	info.key_click = NULL;
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void test_payload_forwarding(void **state)
{
	(void)state;
	obs_source_t *source = obs_source_create_private("interaction_enabled", "interaction", NULL);
	assert_non_null(source);
	expected_data = obs_obj_get_data(source);
	assert_non_null(expected_data);
	struct obs_mouse_event mouse = {.modifiers = UINT32_MAX, .x = INT32_MIN, .y = INT32_MAX};
	char text[] = "key payload";
	struct obs_key_event key = {.modifiers = UINT32_MAX,
				    .text = text,
				    .native_modifiers = 0x87654321,
				    .native_scancode = 0x12345678,
				    .native_vkey = UINT32_MAX};
	expected_mouse = &mouse;
	expected_key = &key;
	for (unsigned pass = 0; pass < 2; pass++) {
		expected_bool = pass != 0;
		expected_type = pass ? INT32_MIN : MOUSE_RIGHT;
		expected_count = pass ? UINT32_MAX : 0;
		expected_x = pass ? INT32_MAX : INT32_MIN;
		expected_y = pass ? INT32_MIN : INT32_MAX;
		dispatch(source);
		for (size_t i = 0; i < 5; i++)
			assert_int_equal(calls[i], pass + 1);
	}
	/* Characterized, not endorsed: event pointers are not validated. */
	expected_mouse = NULL;
	expected_key = NULL;
	dispatch(source);
	for (size_t i = 0; i < 5; i++) {
		assert_int_equal(calls[i], 3);
		calls[i] = 0;
	}
	obs_source_release(source);
}

static void test_gating_and_optional_callbacks(void **state)
{
	(void)state;
	const char *ids[] = {"interaction_disabled", "interaction_missing_callbacks"};
	for (size_t n = 0; n < 2; n++) {
		obs_source_t *source = obs_source_create_private(ids[n], "gated", NULL);
		assert_non_null(source);
		dispatch(source);
		for (size_t i = 0; i < 5; i++)
			assert_int_equal(calls[i], 0);
		obs_source_release(source);
	}
}

static void test_null_source(void **state)
{
	(void)state;
	dispatch(NULL);
	for (size_t i = 0; i < 5; i++)
		assert_int_equal(calls[i], 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_payload_forwarding),
		cmocka_unit_test(test_gating_and_optional_callbacks),
		cmocka_unit_test(test_null_source),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}

/* Characterize the exported getter before extraction/Rust swap (Refs #4755).
 * No video/audio reset: ownership assertions are synchronous, with every
 * source kept alive. Private reference counts are owners minus one. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct fixture {
	obs_canvas_t *canvas;
	obs_source_t *sources[2];
	unsigned signals;
};

static long refs(const obs_source_t *source)
{
	return os_atomic_load_long(&source->context.control->ref.refs);
}

static void channel_change(void *param, calldata_t *data)
{
	(void)data;
	struct fixture *f = param;
	f->signals++;
}

static const char *source_name(void *data)
{
	(void)data;
	return "get output source fixture";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void source_destroy(void *data)
{
	(void)data;
}

static uint32_t source_size(void *data)
{
	(void)data;
	return 16;
}

static int setup(void **state)
{
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct obs_source_info info = {
		.id = "get_output_source_fixture",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
		.get_width = source_size,
		.get_height = source_size,
	};
	obs_register_source(&info);
	struct fixture *f = bzalloc(sizeof(*f));
	*state = f;
	f->canvas = obs_get_main_canvas();
	assert_non_null(f->canvas);
	const char *names[] = {"get output a", "get output b"};
	for (size_t i = 0; i < 2; i++) {
		f->sources[i] = obs_source_create_private(info.id, names[i], NULL);
		assert_non_null(f->sources[i]);
		assert_int_equal(refs(f->sources[i]), 0);
	}
	signal_handler_connect(obs_canvas_get_signal_handler(f->canvas), "channel_change", channel_change, f);
	signal_handler_connect(obs_get_signal_handler(), "channel_change", channel_change, f);
	return 0;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	signal_handler_disconnect(obs_canvas_get_signal_handler(f->canvas), "channel_change", channel_change, f);
	signal_handler_disconnect(obs_get_signal_handler(), "channel_change", channel_change, f);
	for (uint32_t channel = 0; channel < MAX_CHANNELS; channel++)
		obs_set_output_source(channel, NULL);
	for (size_t i = 0; i < 2; i++)
		obs_source_release(f->sources[i]);
	obs_canvas_release(f->canvas);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	bfree(f);
	return 0;
}

static void expect_read(struct fixture *f, uint32_t channel, obs_source_t *expected)
{
	obs_source_t *slots[MAX_CHANNELS];
	memcpy(slots, f->canvas->view.channels, sizeof(slots));
	long before[2], active[2], showing[2];
	for (size_t i = 0; i < 2; i++) {
		before[i] = refs(f->sources[i]);
		active[i] = os_atomic_load_long(&f->sources[i]->activate_refs);
		showing[i] = os_atomic_load_long(&f->sources[i]->show_refs);
	}
	long canvas_refs = os_atomic_load_long(&f->canvas->context.control->ref.refs);
	unsigned signals = f->signals;
	obs_source_t *got = obs_get_output_source(channel);
	assert_ptr_equal(got, expected);
	for (size_t i = 0; i < 2; i++)
		assert_int_equal(refs(f->sources[i]), before[i] + (expected == f->sources[i]));
	obs_source_t *direct = obs_canvas_get_channel(f->canvas, channel);
	assert_ptr_equal(direct, got);
	for (size_t i = 0; i < 2; i++)
		assert_int_equal(refs(f->sources[i]), before[i] + 2 * (expected == f->sources[i]));
	obs_source_release(direct);
	obs_source_release(got);
	assert_memory_equal(f->canvas->view.channels, slots, sizeof(slots));
	assert_int_equal(os_atomic_load_long(&f->canvas->context.control->ref.refs), canvas_refs);
	assert_int_equal(f->signals, signals);
	for (size_t i = 0; i < 2; i++) {
		assert_int_equal(refs(f->sources[i]), before[i]);
		assert_int_equal(os_atomic_load_long(&f->sources[i]->activate_refs), active[i]);
		assert_int_equal(os_atomic_load_long(&f->sources[i]->show_refs), showing[i]);
	}
}

static void empty_and_channel_selection(void **state)
{
	struct fixture *f = *state;
	for (uint32_t channel = 0; channel < MAX_CHANNELS; channel++)
		expect_read(f, channel, NULL);
	obs_set_output_source(0, f->sources[0]);
	obs_set_output_source(3, f->sources[0]);
	obs_set_output_source(MAX_CHANNELS - 1, f->sources[1]);
	for (uint32_t channel = 0; channel < MAX_CHANNELS; channel++) {
		obs_source_t *expected = channel == MAX_CHANNELS - 1    ? f->sources[1]
					 : channel == 0 || channel == 3 ? f->sources[0]
									: NULL;
		expect_read(f, channel, expected);
	}
	obs_set_output_source(0, NULL);
	expect_read(f, 0, NULL);
	expect_read(f, 3, f->sources[0]);
	expect_read(f, MAX_CHANNELS - 1, f->sources[1]);
}

static void acquired_references_survive_clear(void **state)
{
	struct fixture *f = *state;
	obs_source_t *source = f->sources[0];
	obs_set_output_source(0, source);
	assert_int_equal(refs(source), 1);
	obs_source_t *first = obs_get_output_source(0);
	assert_ptr_equal(first, source);
	assert_int_equal(refs(source), 2);
	obs_source_t *second = obs_get_output_source(0);
	assert_ptr_equal(second, source);
	assert_int_equal(refs(source), 3);
	obs_set_output_source(0, NULL);
	assert_int_equal(refs(source), 2);
	assert_string_equal(obs_source_get_name(first), "get output a");
	assert_string_equal(obs_source_get_name(second), "get output a");
	expect_read(f, 0, NULL);
	obs_source_release(first);
	assert_int_equal(refs(source), 1);
	obs_source_release(second);
	assert_int_equal(refs(source), 0);
}

static void invalid_channels(void **state)
{
	struct fixture *f = *state;
	obs_set_output_source(0, f->sources[0]);
	obs_set_output_source(MAX_CHANNELS - 1, f->sources[1]);
#ifdef NDEBUG
	/* Characterized, not endorsed: the downstream assert aborts in debug;
	 * release builds return NULL and leave every slot/reference unchanged. */
	const uint32_t invalid[] = {MAX_CHANNELS, MAX_CHANNELS + 1, UINT32_MAX};
	for (size_t i = 0; i < sizeof(invalid) / sizeof(invalid[0]); i++)
		expect_read(f, invalid[i], NULL);
#endif
	expect_read(f, 0, f->sources[0]);
	expect_read(f, MAX_CHANNELS - 1, f->sources[1]);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(empty_and_channel_selection, setup, teardown),
		cmocka_unit_test_setup_teardown(acquired_references_survive_clear, setup, teardown),
		cmocka_unit_test_setup_teardown(invalid_channels, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

/* Characterize the real exported API before extraction/Rust swap (Refs #4708).
 * No video/audio reset: no render thread can transiently retain the sources.
 * Keep every source alive while checking synchronous counts/signals, and drain
 * destruction only during teardown. Never infer ownership from destroy timing.
 * Private counts are owners minus one, as in test_object_ref.c. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct fixture {
	obs_canvas_t *canvas;
	obs_source_t *sources[3];
	uint32_t channel;
	obs_source_t *previous;
	obs_source_t *requested;
	long signal_refs[3];
	long active_before[3];
	long show_before[3];
	unsigned signals;
	bool override_local;
	bool override_global;
	obs_source_t *local_source;
	obs_source_t *global_source;
};

static long refs(const obs_source_t *source)
{
	return os_atomic_load_long(&source->context.control->ref.refs);
}

static void observe(struct fixture *f, calldata_t *data, bool global)
{
	assert_int_equal(f->signals++, global ? 1 : 0);
	assert_ptr_equal(calldata_ptr(data, "canvas"), f->canvas);
	assert_int_equal(calldata_int(data, "channel"), f->channel);
	assert_ptr_equal(calldata_ptr(data, "prev_source"), f->previous);
	obs_source_t *expected = global && f->override_local ? f->local_source : f->requested;
	assert_ptr_equal(calldata_ptr(data, "source"), expected);
	/* The setter holds the channel mutex here: read its slot directly, never
	 * call a channel getter (which would try to acquire that same mutex). */
	assert_ptr_equal(f->canvas->view.channels[f->channel], f->previous);
	for (size_t i = 0; i < 3; i++) {
		assert_int_equal(refs(f->sources[i]), f->signal_refs[i]);
		assert_int_equal(os_atomic_load_long(&f->sources[i]->activate_refs), f->active_before[i]);
		assert_int_equal(os_atomic_load_long(&f->sources[i]->show_refs), f->show_before[i]);
	}
	if (global && f->override_global)
		calldata_set_ptr(data, "source", f->global_source);
	else if (!global && f->override_local)
		calldata_set_ptr(data, "source", f->local_source);
}

static void local_change(void *param, calldata_t *data)
{
	observe(param, data, false);
}

static void global_change(void *param, calldata_t *data)
{
	observe(param, data, true);
}

static const char *source_name(void *data)
{
	(void)data;
	return "set output source fixture";
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
		.id = "set_output_source_fixture",
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
	assert_int_equal(f->canvas->view.type, MAIN_VIEW);
	const char *names[] = {"set output a", "set output b", "set output c"};
	for (size_t i = 0; i < 3; i++) {
		f->sources[i] = obs_source_create_private(info.id, names[i], NULL);
		assert_non_null(f->sources[i]);
		assert_int_equal(refs(f->sources[i]), 0);
	}
	signal_handler_connect(obs_canvas_get_signal_handler(f->canvas), "channel_change", local_change, f);
	signal_handler_connect(obs_get_signal_handler(), "channel_change", global_change, f);
	return 0;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	signal_handler_disconnect(obs_canvas_get_signal_handler(f->canvas), "channel_change", local_change, f);
	signal_handler_disconnect(obs_get_signal_handler(), "channel_change", global_change, f);
	for (uint32_t channel = 0; channel < MAX_CHANNELS; channel++)
		obs_set_output_source(channel, NULL);
	for (size_t i = 0; i < 3; i++)
		obs_source_release(f->sources[i]);
	obs_canvas_release(f->canvas);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	bfree(f);
	return 0;
}

static void set_channel(struct fixture *f, bool direct, uint32_t channel, obs_source_t *source)
{
	if (direct)
		obs_canvas_set_channel(f->canvas, channel, source);
	else
		obs_set_output_source(channel, source);
}

static void change(struct fixture *f, bool direct, uint32_t channel, obs_source_t *source, obs_source_t *stored,
		   const long during[3], const long after[3])
{
	f->channel = channel;
	f->previous = f->canvas->view.channels[channel];
	f->requested = source;
	f->signals = 0;
	memcpy(f->signal_refs, during, sizeof(f->signal_refs));
	long canvas_refs = os_atomic_load_long(&f->canvas->context.control->ref.refs);
	for (size_t i = 0; i < 3; i++) {
		f->active_before[i] = os_atomic_load_long(&f->sources[i]->activate_refs);
		f->show_before[i] = os_atomic_load_long(&f->sources[i]->show_refs);
	}
	set_channel(f, direct, channel, source);
	assert_int_equal(f->signals, 2);
	assert_ptr_equal(f->canvas->view.channels[channel], stored);
	assert_int_equal(os_atomic_load_long(&f->canvas->context.control->ref.refs), canvas_refs);
	for (size_t i = 0; i < 3; i++) {
		long delta = (stored == f->sources[i]) - (f->previous == f->sources[i]);
		assert_int_equal(refs(f->sources[i]), after[i]);
		assert_int_equal(os_atomic_load_long(&f->sources[i]->activate_refs), f->active_before[i] + delta);
		assert_int_equal(os_atomic_load_long(&f->sources[i]->show_refs), f->show_before[i] + delta);
	}
	obs_source_t *got = obs_get_output_source(channel);
	assert_ptr_equal(got, stored);
	obs_source_release(got);
}

static void unchanged(struct fixture *f, bool direct, uint32_t channel, obs_source_t *source)
{
	obs_source_t *channels[MAX_CHANNELS];
	long before[3], active[3], showing[3];
	memcpy(channels, f->canvas->view.channels, sizeof(channels));
	long canvas_refs = os_atomic_load_long(&f->canvas->context.control->ref.refs);
	for (size_t i = 0; i < 3; i++) {
		before[i] = refs(f->sources[i]);
		active[i] = os_atomic_load_long(&f->sources[i]->activate_refs);
		showing[i] = os_atomic_load_long(&f->sources[i]->show_refs);
	}
	f->signals = 0;
	set_channel(f, direct, channel, source);
	assert_int_equal(f->signals, 0);
	assert_memory_equal(f->canvas->view.channels, channels, sizeof(channels));
	assert_int_equal(os_atomic_load_long(&f->canvas->context.control->ref.refs), canvas_refs);
	for (size_t i = 0; i < 3; i++) {
		assert_int_equal(refs(f->sources[i]), before[i]);
		assert_int_equal(os_atomic_load_long(&f->sources[i]->activate_refs), active[i]);
		assert_int_equal(os_atomic_load_long(&f->sources[i]->show_refs), showing[i]);
	}
}

static void bounds_and_null(void **state)
{
	struct fixture *f = *state;
	unchanged(f, false, 0, NULL);
	unchanged(f, false, MAX_CHANNELS - 1, NULL);
	const long one[] = {1, 0, 0}, two[] = {2, 0, 0}, zero[] = {0, 0, 0};
	change(f, false, 0, f->sources[0], f->sources[0], one, one);
	change(f, false, MAX_CHANNELS - 1, f->sources[0], f->sources[0], two, two);
#ifdef NDEBUG
	/* The library's assert aborts in debug builds. In release builds its
	 * downstream guard rejects out-of-range channels without any effects. */
	const uint32_t invalid[] = {MAX_CHANNELS, MAX_CHANNELS + 1, UINT32_MAX};
	for (size_t i = 0; i < sizeof(invalid) / sizeof(invalid[0]); i++) {
		unchanged(f, false, invalid[i], f->sources[1]);
		unchanged(f, false, invalid[i], NULL);
	}
#endif
	change(f, false, 0, NULL, NULL, two, one);
	change(f, false, MAX_CHANNELS - 1, NULL, NULL, one, zero);
	unchanged(f, false, 0, NULL);
}

static void delegates_like_main_canvas(void **state)
{
	struct fixture *f = *state;
	/* Identical assertions for the direct setter and legacy wrapper, including
	 * one local signal before one global signal and retained old/new refs. */
	for (unsigned direct = 0; direct < 2; direct++) {
		const long a[] = {1, 0, 0}, ab[] = {1, 1, 0}, b[] = {0, 1, 0}, zero[] = {0, 0, 0};
		change(f, direct != 0, 3, f->sources[0], f->sources[0], a, a);
		unchanged(f, direct != 0, 3, f->sources[0]);
		change(f, direct != 0, 3, f->sources[1], f->sources[1], ab, b);
		change(f, direct != 0, 3, NULL, NULL, b, zero);
		unchanged(f, direct != 0, 3, NULL);
	}
}

static void mutable_signal_payload(void **state)
{
	struct fixture *f = *state;
	/* Characterized, not endorsed: callbacks replace the stored source without
	 * transferring the ref taken on the requested source. Supply ownership for
	 * the eventual channel release and explicitly balance the retained ref. */
	assert_ptr_equal(obs_source_get_ref(f->sources[2]), f->sources[2]);
	f->override_local = true;
	f->local_source = f->sources[1];
	f->override_global = true;
	f->global_source = f->sources[2];
	const long ac[] = {1, 0, 1}, a[] = {1, 0, 0}, zero[] = {0, 0, 0};
	change(f, false, 2, f->sources[0], f->sources[2], ac, ac);
	f->override_local = false;
	f->override_global = false;
	unchanged(f, false, 2, f->sources[2]);
	change(f, false, 2, NULL, NULL, ac, a);
	obs_source_release(f->sources[0]);
	assert_int_equal(refs(f->sources[0]), 0);

	/* NULL replacement is also honored; it still retains the requested ref. */
	f->override_global = true;
	f->global_source = NULL;
	change(f, false, 2, f->sources[0], NULL, a, a);
	f->override_global = false;
	unchanged(f, false, 2, NULL);
	obs_source_release(f->sources[0]);
	for (size_t i = 0; i < 3; i++)
		assert_int_equal(refs(f->sources[i]), zero[i]);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(bounds_and_null, setup, teardown),
		cmocka_unit_test_setup_teardown(delegates_like_main_canvas, setup, teardown),
		cmocka_unit_test_setup_teardown(mutable_signal_payload, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

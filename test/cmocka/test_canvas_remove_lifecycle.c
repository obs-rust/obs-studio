/* Characterize obs_canvas_remove() before the Rust swap (Refs #5124).
 * test_canvas_rules_callers::remove_rules pins once-only removal and the main
 * canvas refusal; this adds the retain, signal order, callback-visible flags
 * and the failed-retain path. A NULL canvas is outside the C contract. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

#if !__has_include("obs-canvas-remove.c.h")
#error "canvas removal extraction seam is missing"
#endif

#define REMOVED_BIT (1u << 31)

enum { EVENT_GLOBAL = 1, EVENT_LOCAL = 2 };

struct remove_log {
	obs_canvas_t *canvas;
	int events[4];
	obs_canvas_t *seen[4];
	uint32_t flags[4];
	long refs[4];
	size_t count;
};

static void record(struct remove_log *log, int event, calldata_t *data)
{
	assert_true(log->count < 4);
	obs_canvas_t *canvas = calldata_ptr(data, "canvas");
	log->events[log->count] = event;
	log->seen[log->count] = canvas;
	log->flags[log->count] = canvas->flags;
	log->refs[log->count] = os_atomic_load_long(&canvas->context.control->ref.refs);
	log->count++;
}

static void global_cb(void *param, calldata_t *data)
{
	struct remove_log *log = param;
	/* Other canvases removed elsewhere would land here too. */
	if (calldata_ptr(data, "canvas") == log->canvas)
		record(log, EVENT_GLOBAL, data);
}

static void local_cb(void *param, calldata_t *data)
{
	record(param, EVENT_LOCAL, data);
}

static void connect_log(obs_canvas_t *canvas, struct remove_log *log)
{
	log->canvas = canvas;
	signal_handler_connect(obs_get_signal_handler(), "canvas_remove", global_cb, log);
	signal_handler_connect(obs_canvas_get_signal_handler(canvas), "remove", local_cb, log);
}

static void disconnect_log(obs_canvas_t *canvas, struct remove_log *log)
{
	signal_handler_disconnect(obs_get_signal_handler(), "canvas_remove", global_cb, log);
	signal_handler_disconnect(obs_canvas_get_signal_handler(canvas), "remove", local_cb, log);
}

static long strong_refs(obs_canvas_t *canvas)
{
	return os_atomic_load_long(&canvas->context.control->ref.refs);
}

static long weak_refs(obs_canvas_t *canvas)
{
	return os_atomic_load_long(&canvas->context.control->ref.weak_refs);
}

static void public_canvas_order(void **unused)
{
	(void)unused;
	obs_canvas_t *canvas = obs_canvas_create("remove lifecycle public", NULL, PROGRAM);
	assert_non_null(canvas);
	struct remove_log log = {0};
	connect_log(canvas, &log);
	long refs = strong_refs(canvas);
	long weak = weak_refs(canvas);

	obs_canvas_remove(canvas);

	/* Global first, then the canvas's own signal, both with the REMOVED bit
	 * already written and the retained reference still held. */
	assert_int_equal(log.count, 2);
	assert_int_equal(log.events[0], EVENT_GLOBAL);
	assert_int_equal(log.events[1], EVENT_LOCAL);
	for (size_t i = 0; i < 2; i++) {
		assert_ptr_equal(log.seen[i], canvas);
		assert_int_equal(log.flags[i], PROGRAM | REMOVED_BIT);
		assert_int_equal(log.refs[i], refs + 1);
	}
	/* Released exactly once. */
	assert_int_equal(strong_refs(canvas), refs);
	assert_int_equal(weak_refs(canvas), weak);
	assert_int_equal(canvas->flags, PROGRAM | REMOVED_BIT);
	assert_true(obs_canvas_removed(canvas));

	disconnect_log(canvas, &log);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void private_canvas_local_only(void **unused)
{
	(void)unused;
	obs_canvas_t *canvas = obs_canvas_create_private("remove lifecycle private", NULL, SCENE_REF);
	assert_non_null(canvas);
	struct remove_log log = {0};
	connect_log(canvas, &log);
	long refs = strong_refs(canvas);

	obs_canvas_remove(canvas);

	assert_int_equal(log.count, 1);
	assert_int_equal(log.events[0], EVENT_LOCAL);
	assert_ptr_equal(log.seen[0], canvas);
	assert_int_equal(log.flags[0], SCENE_REF | REMOVED_BIT);
	assert_int_equal(log.refs[0], refs + 1);
	assert_int_equal(strong_refs(canvas), refs);
	assert_int_equal(canvas->flags, SCENE_REF | REMOVED_BIT);

	disconnect_log(canvas, &log);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void main_and_removed_do_nothing(void **unused)
{
	(void)unused;
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_non_null(main_canvas);
	struct remove_log main_log = {0};
	connect_log(main_canvas, &main_log);
	uint32_t main_flags = main_canvas->flags;
	long main_refs = strong_refs(main_canvas);

	obs_canvas_remove(main_canvas);
	assert_int_equal(main_log.count, 0);
	assert_int_equal(main_canvas->flags, main_flags);
	assert_int_equal(strong_refs(main_canvas), main_refs);
	disconnect_log(main_canvas, &main_log);
	obs_canvas_release(main_canvas);

	obs_canvas_t *canvas = obs_canvas_create("remove lifecycle twice", NULL, ACTIVATE);
	assert_non_null(canvas);
	obs_canvas_remove(canvas);
	assert_int_equal(canvas->flags, ACTIVATE | REMOVED_BIT);
	struct remove_log log = {0};
	connect_log(canvas, &log);
	long refs = strong_refs(canvas);
	/* The flags check comes before the retain: no reference is taken. */
	obs_canvas_remove(canvas);
	assert_int_equal(log.count, 0);
	assert_int_equal(canvas->flags, ACTIVATE | REMOVED_BIT);
	assert_int_equal(strong_refs(canvas), refs);
	disconnect_log(canvas, &log);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void failed_retain_does_nothing(void **unused)
{
	(void)unused;
	obs_canvas_t *real = obs_canvas_create_private("remove lifecycle expired", NULL, 0);
	assert_non_null(real);
	obs_weak_canvas_t *weak = obs_canvas_get_weak_canvas(real);
	assert_non_null(weak);
	obs_canvas_release(real);
	assert_int_equal(os_atomic_load_long(&weak->ref.refs), -1);

	/* A public, removable canvas whose control has already expired: the
	 * retain fails, so nothing is signalled and the flags stay unwritten. */
	struct obs_canvas canvas = {0};
	canvas.flags = PROGRAM;
	canvas.context.control = (struct obs_weak_object *)weak;
	struct remove_log log = {.canvas = &canvas};
	signal_handler_connect(obs_get_signal_handler(), "canvas_remove", global_cb, &log);
	long weak_count = os_atomic_load_long(&weak->ref.weak_refs);

	obs_canvas_remove(&canvas);

	assert_int_equal(log.count, 0);
	assert_int_equal(canvas.flags, PROGRAM);
	assert_int_equal(os_atomic_load_long(&weak->ref.refs), -1);
	assert_int_equal(os_atomic_load_long(&weak->ref.weak_refs), weak_count);
	signal_handler_disconnect(obs_get_signal_handler(), "canvas_remove", global_cb, &log);
	obs_weak_canvas_release(weak);
	obs_wait_for_destroy_queue();
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(public_canvas_order),
		cmocka_unit_test(private_canvas_local_only),
		cmocka_unit_test(main_and_removed_do_nothing),
		cmocka_unit_test(failed_retain_does_nothing),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return result;
}

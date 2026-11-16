/* Characterize the static canvas signal helpers through public APIs before
 * extraction/Rust replacement. No video mix or graphics device is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <obs.h>
#include <stdio.h>
#include <string.h>

struct event_log {
	unsigned count;
	char routes[16];
	obs_canvas_t *canvas[16];
	obs_source_t *source[16];
};

struct listener {
	struct event_log *log;
	char route;
	obs_canvas_t *expected_canvas;
};

static void record_event(void *param, calldata_t *data)
{
	struct listener *listener = param;
	struct event_log *log = listener->log;
	unsigned i = log->count++;
	assert_true(i < sizeof(log->routes));
	log->routes[i] = listener->route;
	log->canvas[i] = calldata_ptr(data, "canvas");
	log->source[i] = calldata_ptr(data, "source");
	if (listener->expected_canvas) {
		assert_ptr_equal(log->canvas[i], listener->expected_canvas);
		assert_null(log->source[i]);
	}
}

struct create_listener {
	struct listener global;
	struct listener local;
};

static void created(void *param, calldata_t *data)
{
	struct create_listener *listener = param;
	record_event(&listener->global, data);
	/* Creation requests only the global route. Install a local signal with
	 * that same name during global dispatch so an accidental local dispatch
	 * would be observable, even though canvases normally declare no create. */
	signal_handler_t *handler = obs_canvas_get_signal_handler(calldata_ptr(data, "canvas"));
	assert_true(signal_handler_add(handler, "void canvas_create(ptr canvas)"));
	signal_handler_connect(handler, "canvas_create", record_event, &listener->local);
}

static void global_only_creation(void **state)
{
	(void)state;
	struct event_log log = {0};
	struct create_listener listener = {
		.global = {.log = &log, .route = 'G'},
		.local = {.log = &log, .route = 'L'},
	};
	signal_handler_t *global = obs_get_signal_handler();
	signal_handler_connect(global, "canvas_create", created, &listener);
	obs_canvas_t *pub = obs_canvas_create("signal public create", NULL, 0);
	assert_non_null(pub);
	assert_int_equal(log.count, 1);
	assert_int_equal(log.routes[0], 'G');
	assert_ptr_equal(log.canvas[0], pub);
	assert_null(log.source[0]);
	obs_canvas_t *priv = obs_canvas_create_private("signal private create", NULL, 0);
	assert_non_null(priv);
	assert_int_equal(log.count, 1);
	signal_handler_disconnect(global, "canvas_create", created, &listener);
	signal_handler_disconnect(obs_canvas_get_signal_handler(pub), "canvas_create", record_event, &listener.local);
	obs_canvas_release(pub);
	obs_canvas_release(priv);
	obs_wait_for_destroy_queue();
}

static void lifecycle_routes(bool private)
{
	struct event_log log = {0};
	obs_canvas_t *canvas = private ? obs_canvas_create_private("signal private lifecycle", NULL, 0)
				       : obs_canvas_create("signal public lifecycle", NULL, 0);
	obs_canvas_t *unrelated = obs_canvas_create_private("signal unrelated", NULL, 0);
	assert_non_null(canvas);
	assert_non_null(unrelated);
	struct listener global = {.log = &log, .route = 'G', .expected_canvas = canvas};
	struct listener local = {.log = &log, .route = 'L', .expected_canvas = canvas};
	struct listener other = {.log = &log, .route = 'X', .expected_canvas = unrelated};
	signal_handler_t *gh = obs_get_signal_handler();
	signal_handler_t *ch = obs_canvas_get_signal_handler(canvas);
	signal_handler_t *uh = obs_canvas_get_signal_handler(unrelated);
	signal_handler_connect(gh, "canvas_remove", record_event, &global);
	signal_handler_connect(gh, "canvas_destroy", record_event, &global);
	signal_handler_connect(ch, "remove", record_event, &local);
	signal_handler_connect(ch, "destroy", record_event, &local);
	signal_handler_connect(uh, "remove", record_event, &other);
	signal_handler_connect(uh, "destroy", record_event, &other);

	/* Private canvases still request and receive their local signal. */
	obs_canvas_remove(canvas);
	unsigned per_event = private ? 1 : 2;
	assert_int_equal(log.count, per_event);
	assert_int_equal(log.routes[0], private ? 'L' : 'G');
	if (!private)
		assert_int_equal(log.routes[1], 'L');
	obs_canvas_remove(canvas);
	assert_int_equal(log.count, per_event);

	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
	assert_int_equal(log.count, 2 * per_event);
	assert_int_equal(log.routes[per_event], private ? 'L' : 'G');
	if (!private)
		assert_int_equal(log.routes[per_event + 1], 'L');
	/* Payload identity was checked in each callback while canvas was alive. */
	signal_handler_disconnect(gh, "canvas_remove", record_event, &global);
	signal_handler_disconnect(gh, "canvas_destroy", record_event, &global);
	signal_handler_disconnect(uh, "remove", record_event, &other);
	signal_handler_disconnect(uh, "destroy", record_event, &other);
	obs_canvas_release(unrelated);
	obs_wait_for_destroy_queue();
}

static void public_lifecycle(void **state)
{
	(void)state;
	lifecycle_routes(false);
}

static void private_lifecycle(void **state)
{
	(void)state;
	lifecycle_routes(true);
}

static void source_routes(bool private)
{
	struct event_log log = {0};
	struct listener add = {.log = &log, .route = 'A'};
	struct listener remove = {.log = &log, .route = 'R'};
	struct listener global = {.log = &log, .route = 'G'};
	obs_canvas_t *canvas = private ? obs_canvas_create_private("signal private sources", NULL, 0)
				       : obs_canvas_create("signal public sources", NULL, 0);
	assert_non_null(canvas);
	signal_handler_t *ch = obs_canvas_get_signal_handler(canvas);
	signal_handler_t *gh = obs_get_signal_handler();
	signal_handler_connect(ch, "source_add", record_event, &add);
	signal_handler_connect(ch, "source_remove", record_event, &remove);
	signal_handler_connect(gh, "source_add", record_event, &global);
	signal_handler_connect(gh, "source_remove", record_event, &global);

	obs_scene_t *scene = obs_canvas_scene_create(canvas, "signal payload scene");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	assert_int_equal(log.count, 1);
	assert_int_equal(log.routes[0], 'A');
	assert_ptr_equal(log.canvas[0], canvas);
	assert_ptr_equal(log.source[0], source);
	obs_canvas_scene_remove(scene);
	assert_int_equal(log.count, 2);
	assert_int_equal(log.routes[1], 'R');
	assert_ptr_equal(log.canvas[1], canvas);
	assert_ptr_equal(log.source[1], source);

	signal_handler_disconnect(ch, "source_add", record_event, &add);
	signal_handler_disconnect(ch, "source_remove", record_event, &remove);
	signal_handler_disconnect(gh, "source_add", record_event, &global);
	signal_handler_disconnect(gh, "source_remove", record_event, &global);
	obs_scene_release(scene);
	obs_wait_for_destroy_queue();
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void public_source_payload(void **state)
{
	(void)state;
	source_routes(false);
}

static void private_source_payload(void **state)
{
	(void)state;
	source_routes(true);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 77;
	}
	/* Make an erroneous global source-add dispatch observable. */
	assert_true(signal_handler_add(obs_get_signal_handler(), "void source_add(ptr canvas, ptr source)"));
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(global_only_creation),   cmocka_unit_test(public_lifecycle),
		cmocka_unit_test(private_lifecycle),      cmocka_unit_test(public_source_payload),
		cmocka_unit_test(private_source_payload),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}

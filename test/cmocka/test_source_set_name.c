/* Actual-library characterization before extracting obs_source_set_name.
 * The input path signals global then local, the canvas path local then canvas
 * then (main canvas only) global, and private sources signal only locally.
 * Existing name/dedup/NULL coverage remains in test_context_names. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <string.h>
#include <obs.h>

struct rename_trace {
	obs_source_t *source;
	const char *previous;
	const char *next;
	char events[8];
	size_t count;
};

struct rename_listener {
	struct rename_trace *trace;
	char event;
};

static void renamed(void *param, calldata_t *data)
{
	struct rename_listener *listener = param;
	struct rename_trace *trace = listener->trace;
	assert_ptr_equal(calldata_ptr(data, "source"), trace->source);
	assert_string_equal(calldata_string(data, "prev_name"), trace->previous);
	assert_string_equal(calldata_string(data, "new_name"), trace->next);
	/* The new name is already published when every handler runs. */
	assert_string_equal(obs_source_get_name(trace->source), trace->next);
	assert_true(trace->count + 1 < sizeof(trace->events));
	trace->events[trace->count++] = listener->event;
	trace->events[trace->count] = '\0';
}

static const char *input_name(void *unused)
{
	(void)unused;
	return "source set name input";
}

static void *input_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void input_destroy(void *data)
{
	(void)data;
}

static int setup(void **state)
{
	(void)state;
	/* A startup failure is a failing characterization, never a passing skip. */
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_source_info info = {
		.id = "source_set_name_input",
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = input_name,
		.create = input_create,
		.destroy = input_destroy,
	};
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return 0;
}

static void check_rename(obs_source_t *source, obs_canvas_t *canvas, const char *events)
{
	assert_non_null(source);
	struct rename_trace trace = {.source = source, .previous = "before", .next = "after"};
	struct rename_listener global = {&trace, 'G'}, local = {&trace, 'L'}, canvas_own = {&trace, 'C'};
	signal_handler_t *global_signals = obs_get_signal_handler();
	signal_handler_t *local_signals = obs_source_get_signal_handler(source);
	signal_handler_t *canvas_signals = obs_canvas_get_signal_handler(canvas);
	signal_handler_connect(global_signals, "source_rename", renamed, &global);
	signal_handler_connect(local_signals, "rename", renamed, &local);
	signal_handler_connect(canvas_signals, "source_rename", renamed, &canvas_own);

	obs_source_set_name(source, "before");
	assert_int_equal(trace.count, 0);
	obs_source_set_name(source, "after");
	assert_string_equal(trace.events, events);
	assert_string_equal(obs_source_get_name(source), "after");
	size_t count = trace.count;
	/* Both an equal string and the current name pointer suppress all signals. */
	obs_source_set_name(source, "after");
	obs_source_set_name(source, obs_source_get_name(source));
	assert_int_equal(trace.count, count);

	signal_handler_disconnect(canvas_signals, "source_rename", renamed, &canvas_own);
	signal_handler_disconnect(local_signals, "rename", renamed, &local);
	signal_handler_disconnect(global_signals, "source_rename", renamed, &global);
}

static void public_input_order(void **state)
{
	(void)state;
	obs_canvas_t *canvas = obs_get_main_canvas();
	obs_source_t *source = obs_source_create("source_set_name_input", "before", NULL, NULL);
	check_rename(source, canvas, "GL");
	assert_null(obs_get_source_by_name("before"));
	obs_source_t *found = obs_get_source_by_name("after");
	assert_ptr_equal(found, source);
	obs_source_release(found);
	obs_source_release(source);
	obs_canvas_release(canvas);
}

static void private_input_order(void **state)
{
	(void)state;
	obs_canvas_t *canvas = obs_get_main_canvas();
	obs_source_t *source = obs_source_create_private("source_set_name_input", "before", NULL);
	check_rename(source, canvas, "L");
	assert_null(obs_get_source_by_name("after"));
	obs_source_release(source);
	obs_canvas_release(canvas);
}

static void main_canvas_order(void **state)
{
	(void)state;
	obs_canvas_t *canvas = obs_get_main_canvas();
	obs_scene_t *scene = obs_scene_create("before");
	assert_non_null(scene);
	check_rename(obs_scene_get_source(scene), canvas, "LCG");
	assert_null(obs_canvas_get_scene_by_name(canvas, "before"));
	obs_scene_t *found = obs_canvas_get_scene_by_name(canvas, "after");
	assert_ptr_equal(found, scene);
	obs_scene_release(found);
	obs_scene_release(scene);
	obs_canvas_release(canvas);
}

static void other_canvas_order(void **state)
{
	(void)state;
	obs_canvas_t *canvas = obs_canvas_create("other", NULL, PROGRAM);
	assert_non_null(canvas);
	obs_scene_t *scene = obs_canvas_scene_create(canvas, "before");
	assert_non_null(scene);
	check_rename(obs_scene_get_source(scene), canvas, "LC");
	obs_scene_release(scene);
	obs_canvas_release(canvas);
}

static void private_scene_order(void **state)
{
	(void)state;
	obs_canvas_t *canvas = obs_get_main_canvas();
	obs_scene_t *public_scene = obs_scene_create("after");
	obs_scene_t *scene = obs_scene_create_private("before");
	assert_non_null(public_scene);
	assert_non_null(scene);
	check_rename(obs_scene_get_source(scene), canvas, "L");
	/* A private scene has a canvas but must leave its public table intact. */
	obs_scene_t *found = obs_canvas_get_scene_by_name(canvas, "after");
	assert_ptr_equal(found, public_scene);
	obs_scene_release(found);
	obs_scene_release(scene);
	obs_scene_release(public_scene);
	obs_canvas_release(canvas);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(public_input_order, setup, teardown),
		cmocka_unit_test_setup_teardown(private_input_order, setup, teardown),
		cmocka_unit_test_setup_teardown(main_canvas_order, setup, teardown),
		cmocka_unit_test_setup_teardown(other_canvas_order, setup, teardown),
		cmocka_unit_test_setup_teardown(private_scene_order, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

/* Characterization of obs_source_remove() (#5803), against the hybrid libobs
 * and the C reference through the real core.
 *
 * - A NULL source logs the validity check's line once and signals nothing.
 * - A source not yet removed takes one reference, is marked removed, then
 *   the global "source_remove" signal (public sources only, as
 *   obs_source_dosignal() does) and the source's own "remove" signal fire,
 *   both seeing the source already removed. Only a source with a canvas
 *   that is not private then leaves its canvas: the canvas's "source_remove"
 *   signal fires, the canvas no longer lists it and the source has no canvas
 *   any more. A private source with a canvas keeps it, and the canvas is not
 *   signalled. Then the reference it took is released: the caller's single
 *   release afterwards destroys the source.
 * - A source already removed changes nothing: no signal, no reference taken
 *   or released (the second call of a pair is a no-op).
 * - A source whose last reference is gone (its "destroy" signal, with a
 *   weak reference keeping its reference block) gets no reference from
 *   obs_source_get_ref(), so nothing happens: it is not marked removed and
 *   nothing is signalled.
 *
 * obs_startup() needs a display on Linux (run under xvfb-run); the cases
 * skip when it cannot start. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/dstr.h>

#define BOX_ID "source_remove_box"

static bool obs_up;

/* What happened, in order, while recording: signals and the validity
 * check's log lines, separated by '|'. */
static bool recording;
static struct dstr events;
static int null_lines;

static void record(const char *text)
{
	if (!recording)
		return;
	if (events.len)
		dstr_cat(&events, "|");
	dstr_cat(&events, text);
}

static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (recording) {
		char line[256];
		va_list copy;
		va_copy(copy, args);
		vsnprintf(line, sizeof(line), msg, copy);
		va_end(copy);
		if (lvl == LOG_DEBUG && strstr(line, "obs_source_remove")) {
			record(line);
			null_lines++;
		}
	}
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static void record_start(void)
{
	dstr_free(&events);
	null_lines = 0;
	recording = true;
}

static void record_expect(const char *expected)
{
	recording = false;
	assert_string_equal(events.array ? events.array : "", expected);
	dstr_free(&events);
}

/* "<tag>(<source>,<removed>)" */
static void on_remove(const char *tag, calldata_t *cd)
{
	obs_source_t *source = calldata_ptr(cd, "source");
	struct dstr text = {0};
	dstr_printf(&text, "%s(%s,%d)", tag, source ? obs_source_get_name(source) : "-",
		    source ? (int)obs_source_removed(source) : -1);
	record(text.array);
	dstr_free(&text);
}

static void on_global_remove(void *data, calldata_t *cd)
{
	(void)data;
	on_remove("global", cd);
}

static void on_source_remove(void *data, calldata_t *cd)
{
	(void)data;
	on_remove("source", cd);
}

static void on_canvas_remove(void *data, calldata_t *cd)
{
	(void)data;
	on_remove("canvas", cd);
}

/* Inside the "destroy" signal the last reference is gone. */
static void on_destroy(void *data, calldata_t *cd)
{
	(void)data;
	obs_source_t *source = calldata_ptr(cd, "source");
	obs_source_remove(source);
	struct dstr text = {0};
	dstr_printf(&text, "destroyed(%s,%d)", obs_source_get_name(source), (int)obs_source_removed(source));
	record(text.array);
	dstr_free(&text);
}

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static void *data_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void data_destroy(void *data)
{
	(void)data;
}

static uint32_t box_size(void *data)
{
	(void)data;
	return 64;
}

static void register_types(void)
{
	struct obs_source_info box = {
		.id = BOX_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = box_name,
		.create = data_create,
		.destroy = data_destroy,
		.get_width = box_size,
		.get_height = box_size,
	};
	obs_register_source(&box);
}

static void watch(obs_source_t *source)
{
	signal_handler_connect(obs_source_get_signal_handler(source), "remove", on_source_remove, NULL);
}

static void unwatch(obs_source_t *source)
{
	signal_handler_disconnect(obs_source_get_signal_handler(source), "remove", on_source_remove, NULL);
}

static void watch_canvas(bool on)
{
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_non_null(main_canvas);
	signal_handler_t *handler = obs_canvas_get_signal_handler(main_canvas);
	if (on)
		signal_handler_connect(handler, "source_remove", on_canvas_remove, NULL);
	else
		signal_handler_disconnect(handler, "source_remove", on_canvas_remove, NULL);
	obs_canvas_release(main_canvas);
}

/* Whether the source's canvas is the main canvas (false: no canvas). */
static bool in_main_canvas(obs_source_t *source)
{
	obs_canvas_t *canvas = obs_source_get_canvas(source);
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	bool same = canvas && canvas == main_canvas;
	obs_canvas_release(main_canvas);
	obs_canvas_release(canvas);
	return same;
}

/* Whether the main canvas lists a source by this name. */
static bool main_canvas_lists(const char *name)
{
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	obs_source_t *found = obs_canvas_get_source_by_name(main_canvas, name);
	obs_canvas_release(main_canvas);
	obs_source_release(found);
	return found != NULL;
}

/* The caller's reference is the only one left: one release destroys the
 * source. */
static void release_last(obs_source_t *source)
{
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	assert_false(obs_weak_source_expired(weak));
	obs_source_release(source);
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
}

static bool skip_without_core(void)
{
	if (!obs_up) {
		skip();
		return true;
	}
	return false;
}

static void null_source_logs_once(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	record_start();
	obs_source_remove(NULL);
	assert_int_equal(null_lines, 1);
	record_expect("obs_source_remove: Null 'source' parameter");
}

static void public_source_without_canvas(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	obs_source_t *src = obs_source_create(BOX_ID, "src", NULL, NULL);
	assert_non_null(src);
	watch(src);
	assert_false(obs_source_removed(src));
	assert_false(in_main_canvas(src));

	record_start();
	obs_source_remove(src);
	record_expect("global(src,1)|source(src,1)");
	assert_true(obs_source_removed(src));
	assert_false(in_main_canvas(src));

	unwatch(src);
	release_last(src);
}

static void public_scene_leaves_its_canvas(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	obs_scene_t *scene = obs_scene_create("public scene");
	assert_non_null(scene);
	obs_source_t *src = obs_scene_get_source(scene);
	watch(src);
	watch_canvas(true);
	assert_true(in_main_canvas(src));
	assert_true(main_canvas_lists("public scene"));

	record_start();
	obs_source_remove(src);
	record_expect("global(public scene,1)|source(public scene,1)|canvas(public scene,1)");
	assert_true(obs_source_removed(src));
	assert_false(in_main_canvas(src));
	assert_false(main_canvas_lists("public scene"));

	watch_canvas(false);
	unwatch(src);
	/* The canvas's own reference went with the canvas; the one
	 * obs_source_remove() took is back. */
	release_last(src);
}

static void private_scene_keeps_its_canvas(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	obs_scene_t *scene = obs_scene_create_private("private scene");
	assert_non_null(scene);
	obs_source_t *src = obs_scene_get_source(scene);
	watch(src);
	watch_canvas(true);
	assert_true(in_main_canvas(src));
	assert_false(main_canvas_lists("private scene"));

	record_start();
	obs_source_remove(src);
	/* No global signal for a private source, and no canvas removal. */
	record_expect("source(private scene,1)");
	assert_true(obs_source_removed(src));
	assert_true(in_main_canvas(src));
	assert_false(main_canvas_lists("private scene"));

	watch_canvas(false);
	unwatch(src);
	release_last(src);
}

static void removed_source_changes_nothing(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	obs_scene_t *scene = obs_scene_create("twice");
	assert_non_null(scene);
	obs_source_t *src = obs_scene_get_source(scene);
	watch(src);
	watch_canvas(true);

	record_start();
	obs_source_remove(src);
	record_expect("global(twice,1)|source(twice,1)|canvas(twice,1)");

	record_start();
	obs_source_remove(src);
	obs_source_remove(src);
	record_expect("");
	assert_true(obs_source_removed(src));
	assert_false(in_main_canvas(src));

	watch_canvas(false);
	unwatch(src);
	/* No reference was taken or released by the later calls. */
	release_last(src);
}

static void no_task(void *param)
{
	(void)param;
}

static void released_source_gets_no_reference(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	obs_source_t *src = obs_source_create(BOX_ID, "gone", NULL, NULL);
	assert_non_null(src);
	watch(src);
	signal_handler_connect(obs_source_get_signal_handler(src), "destroy", on_destroy, NULL);
	/* A weak reference keeps the source's reference block alive after its
	 * last strong reference, so obs_source_get_ref() inside the destroy
	 * signal reads it and finds the source expired. */
	obs_weak_source_t *weak = obs_source_get_weak_source(src);

	record_start();
	obs_source_release(src);
	/* The destroy signal runs on the destruction thread. */
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	record_expect("destroyed(gone,0)");
	obs_weak_source_release(weak);
}

static int teardown(void **state)
{
	(void)state;
	recording = false;
	dstr_free(&events);
	if (obs_up)
		obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	return 0;
}

static int group_setup(void **state)
{
	(void)state;
	base_set_log_handler(log_handler, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (obs_up) {
		register_types();
		signal_handler_connect(obs_get_signal_handler(), "source_remove", on_global_remove, NULL);
	} else {
		fprintf(stderr, "obs_startup failed (no display?), skipping\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		signal_handler_disconnect(obs_get_signal_handler(), "source_remove", on_global_remove, NULL);
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(null_source_logs_once, teardown),
		cmocka_unit_test_teardown(public_source_without_canvas, teardown),
		cmocka_unit_test_teardown(public_scene_leaves_its_canvas, teardown),
		cmocka_unit_test_teardown(private_scene_keeps_its_canvas, teardown),
		cmocka_unit_test_teardown(removed_source_changes_nothing, teardown),
		cmocka_unit_test_teardown(released_source_gets_no_reference, teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

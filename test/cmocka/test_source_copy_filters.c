/* Characterization of obs_source_copy_filters() and
 * obs_source_copy_single_filter() (#5792), against the hybrid libobs and
 * the C reference through the real core.
 *
 * - A NULL destination, source or filter logs the validity check's line
 *   once (the destination first) and copies nothing.
 * - obs_source_copy_filters() copies every filter of the source onto the
 *   destination, in front of the destination's own filters and in the
 *   source's order (the copies are added last to first). Each copy is a new
 *   source with the filter's settings and enabled state; a name the
 *   destination already uses gets the next free number ("b 3" when "b" and
 *   "b 2" are taken). Every copy is private, whatever the destination: libobs
 *   creates every filter private. A source without filters copies nothing,
 *   copying a source onto itself copies the filters it had when the call
 *   started, once, and a filter the destination can't take is refused.
 * - obs_source_copy_single_filter() copies one filter the same way, only
 *   when the filter is compatible with the destination (an audio filter
 *   does not go onto a video-only source). The original filter stays on its
 *   own source.
 *
 * obs_source_enum_filters() lists a source's filters from the last added
 * to the first, so the listings below read that way.
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

#define BOX_ID "copy_filters_box"
#define FILTER_ID "copy_filters_filter"
#define AUDIO_FILTER_ID "copy_filters_audio_filter"

static bool obs_up;

/* The LOG_DEBUG lines naming a copy function, while capturing. */
static bool capturing;
static struct dstr captured;
static int captured_lines;

static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (capturing) {
		char line[256];
		va_list copy;
		va_copy(copy, args);
		vsnprintf(line, sizeof(line), msg, copy);
		va_end(copy);
		if (lvl == LOG_DEBUG && strstr(line, "obs_source_copy_")) {
			if (captured.len)
				dstr_cat(&captured, "|");
			dstr_cat(&captured, line);
			captured_lines++;
		}
	}
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static void capture_start(void)
{
	dstr_free(&captured);
	captured_lines = 0;
	capturing = true;
}

static void capture_expect(const char *expected, int lines)
{
	capturing = false;
	assert_string_equal(captured.array ? captured.array : "", expected);
	assert_int_equal(captured_lines, lines);
	dstr_free(&captured);
}

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static const char *filter_name(void *type_data)
{
	(void)type_data;
	return "copy test filter";
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
	struct obs_source_info filter = {
		.id = FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = filter_name,
		.create = data_create,
		.destroy = data_destroy,
	};
	struct obs_source_info audio_filter = {
		.id = AUDIO_FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = filter_name,
		.create = data_create,
		.destroy = data_destroy,
	};
	obs_register_source(&box);
	obs_register_source(&filter);
	obs_register_source(&audio_filter);
}

static obs_source_t *add_filter(obs_source_t *source, const char *id, const char *name, bool enabled, int value)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "value", value);
	obs_source_t *filter = obs_source_create_private(id, name, settings);
	obs_data_release(settings);
	assert_non_null(filter);
	obs_source_set_enabled(filter, enabled);
	obs_source_filter_add(source, filter);
	obs_source_release(filter);
	return filter;
}

static obs_source_t *box(const char *name, bool private)
{
	obs_source_t *source = private ? obs_source_create_private(BOX_ID, name, NULL)
				       : obs_source_create(BOX_ID, name, NULL, NULL);
	assert_non_null(source);
	return source;
}

static void drop(obs_source_t *source)
{
	obs_source_remove(source);
	obs_source_release(source);
}

struct listing {
	struct dstr text;
	obs_source_t *parent;
};

static void describe(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct listing *l = param;
	obs_data_t *settings = obs_source_get_settings(child);
	(void)parent;
	if (l->text.len)
		dstr_cat(&l->text, ",");
	dstr_catf(&l->text, "%s:%s:%s:%lld", obs_source_get_name(child), obs_source_enabled(child) ? "on" : "off",
		  obs_obj_is_private(child) ? "private" : "public", (long long)obs_data_get_int(settings, "value"));
	obs_data_release(settings);
}

/* The source's filters, in order, as "name:enabled:private:value,...". */
static void expect_filters(obs_source_t *source, const char *expected)
{
	struct listing l = {0};
	obs_source_enum_filters(source, describe, &l);
	assert_string_equal(l.text.array ? l.text.array : "", expected);
	dstr_free(&l.text);
}

static bool skip_without_core(void)
{
	if (!obs_up) {
		skip();
		return true;
	}
	return false;
}

static void null_arguments(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src", false);
	obs_source_t *dst = box("dst", false);
	obs_source_t *filter = add_filter(src, FILTER_ID, "a", true, 1);

	capture_start();
	obs_source_copy_filters(NULL, src);
	capture_expect("obs_source_copy_filters: Null 'dst' parameter", 1);
	capture_start();
	obs_source_copy_filters(dst, NULL);
	capture_expect("obs_source_copy_filters: Null 'src' parameter", 1);
	capture_start();
	obs_source_copy_filters(NULL, NULL);
	capture_expect("obs_source_copy_filters: Null 'dst' parameter", 1);
	capture_start();
	obs_source_copy_single_filter(NULL, filter);
	capture_expect("obs_source_copy_single_filter: Null 'dst' parameter", 1);
	capture_start();
	obs_source_copy_single_filter(dst, NULL);
	capture_expect("obs_source_copy_single_filter: Null 'filter' parameter", 1);
	capture_start();
	obs_source_copy_single_filter(NULL, NULL);
	capture_expect("obs_source_copy_single_filter: Null 'dst' parameter", 1);

	expect_filters(src, "a:on:private:1");
	expect_filters(dst, "");
	drop(dst);
	drop(src);
}

static void copy_filters_order_names_and_state(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src", false);
	obs_source_t *dst = box("dst", false);
	/* Each add goes to the front: the list is c, b, a, listed a, b, c. */
	add_filter(src, FILTER_ID, "a", true, 1);
	add_filter(src, FILTER_ID, "b", false, 2);
	add_filter(src, FILTER_ID, "c", true, 3);
	add_filter(dst, FILTER_ID, "b", true, 9);
	add_filter(dst, FILTER_ID, "b 2", true, 8);

	capture_start();
	obs_source_copy_filters(dst, src);
	capture_expect("", 0);

	expect_filters(src, "a:on:private:1,b:off:private:2,c:on:private:3");
	expect_filters(dst, "b:on:private:9,b 2:on:private:8,a:on:private:1,b 3:off:private:2,c:on:private:3");
	drop(dst);
	drop(src);
}

static void copy_filters_private_destination(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src", false);
	obs_source_t *dst = box("dst", true);
	add_filter(src, FILTER_ID, "a", false, 1);
	add_filter(src, FILTER_ID, "b", true, 2);

	obs_source_copy_filters(dst, src);

	expect_filters(dst, "a:off:private:1,b:on:private:2");
	drop(dst);
	drop(src);
}

static void copy_filters_none(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src", false);
	obs_source_t *dst = box("dst", false);
	add_filter(dst, FILTER_ID, "a", true, 1);

	obs_source_copy_filters(dst, src);

	expect_filters(src, "");
	expect_filters(dst, "a:on:private:1");
	drop(dst);
	drop(src);
}

static void copy_filters_onto_itself(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src", false);
	add_filter(src, FILTER_ID, "a", true, 1);
	add_filter(src, FILTER_ID, "b", false, 2);

	obs_source_copy_filters(src, src);

	expect_filters(src, "a:on:private:1,b:off:private:2,a 2:on:private:1,b 2:off:private:2");
	drop(src);
}

static void copy_filters_skips_incompatible(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src", false);
	obs_source_t *dst = box("dst", false);
	/* The box takes no audio filter: obs_source_filter_add() refuses it,
	 * on the source and on the destination alike. */
	add_filter(src, AUDIO_FILTER_ID, "sound", true, 5);
	add_filter(src, FILTER_ID, "a", true, 1);

	obs_source_copy_filters(dst, src);

	expect_filters(src, "a:on:private:1");
	expect_filters(dst, "a:on:private:1");
	drop(dst);
	drop(src);
}

static void copy_single_filter_private_and_named(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src", false);
	obs_source_t *dst = box("dst", false);
	obs_source_t *a = add_filter(src, FILTER_ID, "a", false, 1);
	add_filter(src, FILTER_ID, "b", true, 2);
	add_filter(dst, FILTER_ID, "a", true, 7);

	capture_start();
	obs_source_copy_single_filter(dst, a);
	capture_expect("", 0);

	expect_filters(src, "a:off:private:1,b:on:private:2");
	expect_filters(dst, "a:on:private:7,a 2:off:private:1");
	assert_ptr_equal(obs_filter_get_parent(a), src);

	/* Onto its own source: a second copy next to it. */
	obs_source_copy_single_filter(src, a);
	expect_filters(src, "a:off:private:1,b:on:private:2,a 2:off:private:1");
	drop(dst);
	drop(src);
}

static void copy_single_filter_incompatible(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *dst = box("dst", false);
	obs_data_t *settings = obs_data_create();
	obs_source_t *sound = obs_source_create_private(AUDIO_FILTER_ID, "sound", settings);
	obs_data_release(settings);
	assert_non_null(sound);
	add_filter(dst, FILTER_ID, "a", true, 1);

	capture_start();
	obs_source_copy_single_filter(dst, sound);
	capture_expect("", 0);

	expect_filters(dst, "a:on:private:1");
	obs_source_release(sound);
	drop(dst);
}

static void no_task(void *param)
{
	(void)param;
}

static int teardown(void **state)
{
	(void)state;
	capturing = false;
	dstr_free(&captured);
	if (obs_up)
		obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	return 0;
}

static int group_setup(void **state)
{
	(void)state;
	base_set_log_handler(log_handler, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (obs_up)
		register_types();
	else
		fprintf(stderr, "obs_startup failed (no display?), skipping\n");
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(null_arguments, teardown),
		cmocka_unit_test_teardown(copy_filters_order_names_and_state, teardown),
		cmocka_unit_test_teardown(copy_filters_private_destination, teardown),
		cmocka_unit_test_teardown(copy_filters_none, teardown),
		cmocka_unit_test_teardown(copy_filters_onto_itself, teardown),
		cmocka_unit_test_teardown(copy_filters_skips_incompatible, teardown),
		cmocka_unit_test_teardown(copy_single_filter_private_and_named, teardown),
		cmocka_unit_test_teardown(copy_single_filter_incompatible, teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

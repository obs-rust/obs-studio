/* Characterization of obs_source_filter_remove() (#5795), against the
 * hybrid libobs and the C reference through the real core.
 *
 * - A NULL source or filter logs the validity check's line once (the source
 *   first) and removes, signals and releases nothing.
 * - A filter that is not on the source's list changes nothing: no signal, no
 *   callback, no release, and the list and targets stay as they were.
 * - A listed filter is taken off the list. When it was not the first entry,
 *   the entry before it takes over its target. Then, with filter_mutex
 *   released, the global "source_filter_remove" signal and the source's own
 *   "filter_remove" signal fire (both with the source and the filter, the
 *   list already without the filter, the filter's parent still set), the
 *   debug line "- filter '<name>' (<id>) removed from source '<name>'" is
 *   logged, the filter type's filter_remove callback runs with the parent
 *   when the type has one, the filter's parent and target are cleared, and
 *   the list's reference to the filter is released once.
 * - Destroying a source removes its filters the same way, from the first
 *   entry of the list.
 *
 * Each add puts the filter at the front of the list: after adding a, b and c
 * the list is c, b, a, and c targets b, b targets a, a targets the source.
 *
 * Whether filter_mutex is free when a signal or the callback runs is checked
 * from a second thread (the mutex is recursive, so the calling thread could
 * always take it): obs_source_filter_get_index() takes the mutex there.
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
#include <util/bmem.h>
#include <util/dstr.h>
#include <util/threading.h>

#define BOX_ID "filter_remove_box"
#define FILTER_ID "filter_remove_filter"
#define PLAIN_FILTER_ID "filter_remove_plain_filter"

static bool obs_up;

/* What happened, in order, while recording: signals, callbacks and the log
 * lines about filter removal, separated by '|'. */
static bool recording;
static struct dstr events;
static int null_lines;

/* A probe thread that is still waiting for filter_mutex. */
static pthread_t stuck_probe;
static bool probe_stuck;

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
		if (lvl == LOG_DEBUG && strstr(line, "obs_source_filter_remove")) {
			record(line);
			null_lines++;
		} else if (lvl == LOG_DEBUG && strstr(line, "removed from source")) {
			record(line);
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

struct probe {
	obs_source_t *source;
	os_event_t *done;
};

static void *probe_thread(void *param)
{
	struct probe *p = param;
	obs_source_filter_get_index(p->source, p->source);
	os_event_signal(p->done);
	return NULL;
}

/* Whether another thread can take source's filter_mutex now. */
static bool mutex_free(obs_source_t *source)
{
	struct probe *p = bzalloc(sizeof(*p));
	pthread_t thread;

	p->source = source;
	assert_int_equal(os_event_init(&p->done, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(pthread_create(&thread, NULL, probe_thread, p), 0);
	bool free = os_event_timedwait(p->done, 2000) == 0;
	if (free) {
		pthread_join(thread, NULL);
		os_event_destroy(p->done);
		bfree(p);
	} else {
		/* Joined once the call under test has returned; the probe's
		 * state is left to it. */
		stuck_probe = thread;
		probe_stuck = true;
	}
	return free;
}

static void join_stuck_probe(void)
{
	if (probe_stuck) {
		pthread_join(stuck_probe, NULL);
		probe_stuck = false;
	}
}

static const char *name_of(obs_source_t *source)
{
	return source ? obs_source_get_name(source) : "-";
}

/* "<tag>(<source>,<filter>,<filter's parent>,<count>,<free|held>)" */
static void on_remove(const char *tag, calldata_t *cd)
{
	obs_source_t *source = calldata_ptr(cd, "source");
	obs_source_t *filter = calldata_ptr(cd, "filter");
	struct dstr text = {0};
	dstr_printf(&text, "%s(%s,%s,%s,%d,%s)", tag, name_of(source), name_of(filter),
		    name_of(obs_filter_get_parent(filter)), (int)obs_source_filter_count(source),
		    mutex_free(source) ? "free" : "held");
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

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static const char *filter_name(void *type_data)
{
	(void)type_data;
	return "remove test filter";
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

/* "callback(<filter>,<parent argument>,<filter's parent>,<target>,<free|held>)" */
static void filter_removed(void *data, obs_source_t *parent)
{
	obs_source_t *filter = data;
	struct dstr text = {0};
	dstr_printf(&text, "callback(%s,%s,%s,%s,%s)", name_of(filter), name_of(parent),
		    name_of(obs_filter_get_parent(filter)), name_of(obs_filter_get_target(filter)),
		    parent && mutex_free(parent) ? "free" : "held");
	record(text.array);
	dstr_free(&text);
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
		.filter_remove = filter_removed,
	};
	struct obs_source_info plain_filter = {
		.id = PLAIN_FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = filter_name,
		.create = data_create,
		.destroy = data_destroy,
	};
	obs_register_source(&box);
	obs_register_source(&filter);
	obs_register_source(&plain_filter);
}

static obs_source_t *box(const char *name)
{
	obs_source_t *source = obs_source_create(BOX_ID, name, NULL, NULL);
	assert_non_null(source);
	signal_handler_connect(obs_source_get_signal_handler(source), "filter_remove", on_source_remove, NULL);
	return source;
}

/* A new filter, with a reference the caller owns. */
static obs_source_t *new_filter(const char *id, const char *name)
{
	obs_source_t *filter = obs_source_create_private(id, name, NULL);
	assert_non_null(filter);
	return filter;
}

/* A filter added to source, with a reference the caller owns too. */
static obs_source_t *add_filter(obs_source_t *source, const char *id, const char *name)
{
	obs_source_t *filter = new_filter(id, name);
	obs_source_filter_add(source, filter);
	assert_ptr_equal(obs_filter_get_parent(filter), source);
	return filter;
}

static void drop(obs_source_t *source)
{
	signal_handler_disconnect(obs_source_get_signal_handler(source), "filter_remove", on_source_remove, NULL);
	obs_source_remove(source);
	obs_source_release(source);
}

static void describe(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct dstr *text = param;
	(void)parent;
	struct dstr entry = {0};
	dstr_printf(&entry, "%s>%s", obs_source_get_name(child), name_of(obs_filter_get_target(child)));
	/* obs_source_enum_filters() walks the list from the last entry. */
	if (text->len)
		dstr_insert(text, 0, ",");
	dstr_insert(text, 0, entry.array);
	dstr_free(&entry);
}

/* The source's list from the first entry, each as "<name>><target>". */
static void expect_chain(obs_source_t *source, const char *expected)
{
	struct dstr listed = {0};
	obs_source_enum_filters(source, describe, &listed);
	assert_string_equal(listed.array ? listed.array : "", expected);
	dstr_free(&listed);
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
	obs_source_t *src = box("src");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	obs_weak_source_t *weak = obs_source_get_weak_source(a);

	record_start();
	obs_source_filter_remove(NULL, a);
	obs_source_filter_remove(src, NULL);
	obs_source_filter_remove(NULL, NULL);
	assert_int_equal(null_lines, 3);
	record_expect("obs_source_filter_remove: Null 'source' parameter|"
		      "obs_source_filter_remove: Null 'filter' parameter|"
		      "obs_source_filter_remove: Null 'source' parameter");

	expect_chain(src, "a>src");
	assert_ptr_equal(obs_filter_get_parent(a), src);
	/* Nothing was released: the list still holds its reference. */
	obs_source_release(a);
	assert_false(obs_weak_source_expired(weak));
	drop(src);
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
}

static void unlisted_filter_changes_nothing(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *other = box("other");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	obs_source_t *b = add_filter(src, FILTER_ID, "b");
	obs_source_t *loose = new_filter(FILTER_ID, "loose");
	obs_source_t *elsewhere = add_filter(other, FILTER_ID, "elsewhere");
	obs_weak_source_t *weak_loose = obs_source_get_weak_source(loose);
	obs_weak_source_t *weak_elsewhere = obs_source_get_weak_source(elsewhere);

	record_start();
	obs_source_filter_remove(src, loose);
	obs_source_filter_remove(src, elsewhere);
	/* A source is never on its own list. */
	obs_source_filter_remove(src, src);
	record_expect("");

	expect_chain(src, "b>a,a>src");
	expect_chain(other, "elsewhere>other");
	assert_null(obs_filter_get_parent(loose));
	assert_ptr_equal(obs_filter_get_parent(elsewhere), other);

	/* Nothing was released: the caller's references are the last ones. */
	obs_source_release(loose);
	assert_true(obs_weak_source_expired(weak_loose));
	obs_source_release(elsewhere);
	assert_false(obs_weak_source_expired(weak_elsewhere));
	obs_source_release(a);
	obs_source_release(b);
	drop(other);
	assert_true(obs_weak_source_expired(weak_elsewhere));
	drop(src);
	obs_weak_source_release(weak_loose);
	obs_weak_source_release(weak_elsewhere);
}

static void remove_middle_retargets_the_previous_entry(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	obs_source_t *b = add_filter(src, FILTER_ID, "b");
	obs_source_t *c = add_filter(src, FILTER_ID, "c");
	obs_weak_source_t *weak = obs_source_get_weak_source(b);
	expect_chain(src, "c>b,b>a,a>src");

	record_start();
	obs_source_filter_remove(src, b);
	record_expect("global(src,b,src,2,free)|source(src,b,src,2,free)|"
		      "- filter 'b' (" FILTER_ID ") removed from source 'src'|"
		      "callback(b,src,src,a,free)");

	expect_chain(src, "c>a,a>src");
	assert_null(obs_filter_get_parent(b));
	assert_null(obs_filter_get_target(b));
	/* One release: the caller's reference is the last one. */
	assert_false(obs_weak_source_expired(weak));
	obs_source_release(b);
	assert_true(obs_weak_source_expired(weak));

	obs_source_release(a);
	obs_source_release(c);
	drop(src);
	obs_weak_source_release(weak);
}

static void remove_first_entry_retargets_nothing(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	obs_source_t *b = add_filter(src, FILTER_ID, "b");
	obs_source_t *c = add_filter(src, FILTER_ID, "c");

	record_start();
	obs_source_filter_remove(src, c);
	record_expect("global(src,c,src,2,free)|source(src,c,src,2,free)|"
		      "- filter 'c' (" FILTER_ID ") removed from source 'src'|"
		      "callback(c,src,src,b,free)");
	expect_chain(src, "b>a,a>src");
	assert_null(obs_filter_get_parent(c));
	assert_null(obs_filter_get_target(c));

	/* A second removal of the same filter changes nothing. */
	record_start();
	obs_source_filter_remove(src, c);
	record_expect("");
	expect_chain(src, "b>a,a>src");

	obs_source_release(a);
	obs_source_release(b);
	obs_source_release(c);
	drop(src);
}

static void remove_last_entry_hands_over_the_source(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	obs_source_t *b = add_filter(src, PLAIN_FILTER_ID, "b");
	obs_source_t *c = add_filter(src, FILTER_ID, "c");

	record_start();
	obs_source_filter_remove(src, a);
	record_expect("global(src,a,src,2,free)|source(src,a,src,2,free)|"
		      "- filter 'a' (" FILTER_ID ") removed from source 'src'|"
		      "callback(a,src,src,src,free)");
	expect_chain(src, "c>b,b>src");

	/* A type without a filter_remove callback: everything but the call. */
	record_start();
	obs_source_filter_remove(src, b);
	record_expect("global(src,b,src,1,free)|source(src,b,src,1,free)|"
		      "- filter 'b' (" PLAIN_FILTER_ID ") removed from source 'src'");
	expect_chain(src, "c>src");
	assert_null(obs_filter_get_parent(b));
	assert_null(obs_filter_get_target(b));

	/* The only filter. */
	record_start();
	obs_source_filter_remove(src, c);
	record_expect("global(src,c,src,0,free)|source(src,c,src,0,free)|"
		      "- filter 'c' (" FILTER_ID ") removed from source 'src'|"
		      "callback(c,src,src,src,free)");
	expect_chain(src, "");

	obs_source_release(a);
	obs_source_release(b);
	obs_source_release(c);
	drop(src);
}

static void destroy_removes_every_filter(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	obs_source_t *b = add_filter(src, PLAIN_FILTER_ID, "b");
	obs_weak_source_t *weak_a = obs_source_get_weak_source(a);
	obs_weak_source_t *weak_b = obs_source_get_weak_source(b);
	obs_source_release(a);
	obs_source_release(b);

	record_start();
	/* The source's own handler stays connected until it is destroyed. */
	obs_source_remove(src);
	obs_source_release(src);
	record_expect("global(src,b,src,1,free)|source(src,b,src,1,free)|"
		      "- filter 'b' (" PLAIN_FILTER_ID ") removed from source 'src'|"
		      "global(src,a,src,0,free)|source(src,a,src,0,free)|"
		      "- filter 'a' (" FILTER_ID ") removed from source 'src'|"
		      "callback(a,src,src,src,free)");
	assert_true(obs_weak_source_expired(weak_a));
	assert_true(obs_weak_source_expired(weak_b));
	obs_weak_source_release(weak_a);
	obs_weak_source_release(weak_b);
}

static void no_task(void *param)
{
	(void)param;
}

static int teardown(void **state)
{
	(void)state;
	recording = false;
	dstr_free(&events);
	join_stuck_probe();
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
		signal_handler_connect(obs_get_signal_handler(), "source_filter_remove", on_global_remove, NULL);
	} else {
		fprintf(stderr, "obs_startup failed (no display?), skipping\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		signal_handler_disconnect(obs_get_signal_handler(), "source_filter_remove", on_global_remove, NULL);
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(null_arguments, teardown),
		cmocka_unit_test_teardown(unlisted_filter_changes_nothing, teardown),
		cmocka_unit_test_teardown(remove_middle_retargets_the_previous_entry, teardown),
		cmocka_unit_test_teardown(remove_first_entry_retargets_nothing, teardown),
		cmocka_unit_test_teardown(remove_last_entry_hands_over_the_source, teardown),
		cmocka_unit_test_teardown(destroy_removes_every_filter, teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

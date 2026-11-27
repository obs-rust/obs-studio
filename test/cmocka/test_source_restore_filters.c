/* Characterization of obs_source_restore_filters() (#5801), against the
 * hybrid libobs and the C reference through the real core.
 *
 * - An invalid source or a NULL array logs the validity check's line once
 *   (the source checked first) with filter_mutex free, and locks or changes
 *   nothing.
 * - Every current filter is taken off the list: while the backed-up filters
 *   are restored, the source's list is empty and each old filter has no
 *   parent and no target, and filter_mutex is free.
 * - For each array item in order, the first current filter whose name
 *   matches the item's "name" is reused (the same filter, whatever the
 *   item's "id"); an unnamed current filter never matches, not even an
 *   item named "" (no filter can have a blank name). An item with no
 *   match is loaded from its data (a filter of the item's id and name; the
 *   C reference loads it public, hybrid libobs private, an intentional
 *   difference of the filter chain rules (#2981), so neither is checked).
 *   Each restored filter targets the next one, the last the source; while a
 *   later item loads, the earlier ones already target their successor but
 *   have no parent: every parent is set to the source only once every
 *   target is (#2172).
 * - The new list holds one reference to each entry; a duplicate name in the
 *   array reuses the same current filter each time, so it is listed twice.
 * - Afterwards each old filter is released once, in old order, with the new
 *   list in place and filter_mutex free; a filter not reused goes away.
 * - No filter signal fires and, when every item is reused, nothing is
 *   logged.
 *
 * A current filter that can't be referenced (its last reference gone) is
 * not reachable here: a listed filter holds the list's reference until the
 * restore releases it. The Rust port's Tier 1 and Tier 3 tests cover it.
 *
 * Whether filter_mutex is free is checked from a second thread (the mutex
 * is recursive, so the calling thread could always take it):
 * obs_source_filter_get_index() takes the mutex there.
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

#define BOX_ID "restore_filters_box"
#define FILTER_ID "restore_filters_filter"
#define OTHER_FILTER_ID "restore_filters_other_filter"
#define WATCH_ID "restore_filters_watch"

static bool obs_up;

/* What happened, in order, while recording: filter signals of the source
 * under test, filter loads, old filter releases and the validity lines,
 * separated by '|'. */
static bool recording;
static struct dstr events;
static int null_lines;
static int all_lines;

/* The source under test. */
static obs_source_t *watched;
/* A filter whose parent and target a load reports. */
static obs_source_t *watched_filter;

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
		assert_false(probe_stuck);
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

static const char *lock_state(void)
{
	if (!watched)
		return "-";
	return mutex_free(watched) ? "free" : "held";
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
		all_lines++;
		if (lvl == LOG_DEBUG && strstr(line, "obs_source_restore_filters")) {
			struct dstr text = {0};
			dstr_printf(&text, "%s(%s)", line, lock_state());
			record(text.array);
			dstr_free(&text);
			null_lines++;
		}
	}
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static void record_start(obs_source_t *source)
{
	dstr_free(&events);
	null_lines = 0;
	all_lines = 0;
	watched = source;
	recording = true;
}

static void record_expect(const char *expected)
{
	recording = false;
	watched = NULL;
	watched_filter = NULL;
	join_stuck_probe();
	assert_string_equal(events.array ? events.array : "", expected);
	dstr_free(&events);
}

static const char *name_of(obs_source_t *source)
{
	if (!source)
		return "-";
	const char *name = obs_source_get_name(source);
	return name ? name : "(unnamed)";
}

/* "<signal>(<source>,<filter>)", for the source under test only. */
static void on_filter_signal(void *data, calldata_t *cd)
{
	obs_source_t *source = calldata_ptr(cd, "source");
	obs_source_t *filter = calldata_ptr(cd, "filter");
	if (!recording || !watched || source != watched)
		return;
	struct dstr text = {0};
	dstr_printf(&text, "%s(%s,%s)", (const char *)data, name_of(source), name_of(filter));
	record(text.array);
	dstr_free(&text);
}

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static const char *filter_name(void *type_data)
{
	(void)type_data;
	return "restore test filter";
}

/* "load(<name>,<count>,<free|held>,<watched filter's parent>><target>)":
 * a filter created while recording, the source's filter count and lock
 * state then. */
static void *filter_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	if (recording && watched) {
		struct dstr text = {0};
		/* A blank name gets a generated one, numbered across the run. */
		const char *name = name_of(source);
		if (strncmp(name, "__unnamed", 9) == 0)
			name = "generated";
		dstr_printf(&text, "load(%s,%d,%s,%s>%s)", name, (int)obs_source_filter_count(watched), lock_state(),
			    name_of(obs_filter_get_parent(watched_filter)),
			    name_of(obs_filter_get_target(watched_filter)));
		record(text.array);
		dstr_free(&text);
	}
	return source;
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

/* "released(<parent>,<count>,<free|held>)": runs when parent, an old filter
 * this watch filter sits on, is destroyed, with the count and lock state of
 * the source under test then. */
static void watch_removed(void *data, obs_source_t *parent)
{
	(void)data;
	if (!recording || !watched)
		return;
	struct dstr text = {0};
	dstr_printf(&text, "released(%s,%d,%s)", name_of(parent), (int)obs_source_filter_count(watched), lock_state());
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
		.create = filter_create,
		.destroy = data_destroy,
	};
	struct obs_source_info other_filter = {
		.id = OTHER_FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = filter_name,
		.create = filter_create,
		.destroy = data_destroy,
	};
	struct obs_source_info watch = {
		.id = WATCH_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = filter_name,
		.create = data_create,
		.destroy = data_destroy,
		.filter_remove = watch_removed,
	};
	obs_register_source(&box);
	obs_register_source(&filter);
	obs_register_source(&other_filter);
	obs_register_source(&watch);
}

/* The filter signals recorded; each name is also its handler's data. */
static const char *const source_signals[] = {"filter_add", "filter_remove", "reorder_filters"};
static const char *const global_signals[] = {"source_filter_add", "source_filter_remove"};
#define SIGNAL_COUNT(list) (sizeof(list) / sizeof((list)[0]))

static obs_source_t *box(const char *name)
{
	obs_source_t *source = obs_source_create(BOX_ID, name, NULL, NULL);
	assert_non_null(source);
	signal_handler_t *handler = obs_source_get_signal_handler(source);
	for (size_t i = 0; i < SIGNAL_COUNT(source_signals); i++)
		signal_handler_connect(handler, source_signals[i], on_filter_signal, (void *)source_signals[i]);
	return source;
}

static void drop(obs_source_t *source)
{
	signal_handler_t *handler = obs_source_get_signal_handler(source);
	for (size_t i = 0; i < SIGNAL_COUNT(source_signals); i++)
		signal_handler_disconnect(handler, source_signals[i], on_filter_signal, (void *)source_signals[i]);
	obs_source_remove(source);
	obs_source_release(source);
}

/* A new filter on source (added at the front of its list), with a
 * reference the caller owns. */
static obs_source_t *add_filter(obs_source_t *source, const char *id, const char *name)
{
	obs_source_t *filter = obs_source_create_private(id, name, NULL);
	assert_non_null(filter);
	obs_source_filter_add(source, filter);
	return filter;
}

/* Puts a watch filter on filter, so its destruction is recorded. */
static void watch(obs_source_t *filter)
{
	obs_source_t *w = obs_source_create_private(WATCH_ID, "watch", NULL);
	assert_non_null(w);
	obs_source_filter_add(filter, w);
	obs_source_release(w);
}

static void describe(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct dstr *text = param;
	struct dstr entry = {0};
	dstr_printf(&entry, "%s>%s", name_of(child), name_of(obs_filter_get_target(child)));
	/* Every listed filter has the source as its parent. */
	assert_ptr_equal(obs_filter_get_parent(child), parent);
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

struct find {
	const char *name;
	obs_source_t *found;
};

static void find_named(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct find *f = param;
	(void)parent;
	if (!f->found && strcmp(name_of(child), f->name) == 0)
		f->found = child;
}

/* A new reference to the listed filter called name. */
static obs_source_t *listed(obs_source_t *source, const char *name)
{
	struct find f = {name, NULL};
	obs_source_enum_filters(source, find_named, &f);
	assert_non_null(f.found);
	return obs_source_get_ref(f.found);
}

/* An array of backed-up filters: pairs of name and id, ending in NULL. */
static obs_data_array_t *backup_of(const char *name, ...)
{
	obs_data_array_t *array = obs_data_array_create();
	va_list args;
	va_start(args, name);
	for (const char *n = name; n; n = va_arg(args, const char *)) {
		const char *id = va_arg(args, const char *);
		obs_data_t *item = obs_data_create();
		obs_data_set_string(item, "name", n);
		obs_data_set_string(item, "id", id);
		obs_data_set_string(item, "versioned_id", id);
		obs_data_array_push_back(array, item);
		obs_data_release(item);
	}
	va_end(args);
	return array;
}

/* Whether the list's reference is the only one left on filter: dropping
 * the caller's keeps it alive, removing it from source frees it. */
static void expect_one_list_reference(obs_source_t *source, obs_source_t *filter)
{
	obs_weak_source_t *weak = obs_source_get_weak_source(filter);
	obs_source_release(filter);
	assert_false(obs_weak_source_expired(weak));
	obs_source_filter_remove(source, filter);
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

static void null_arguments(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	obs_data_array_t *array = backup_of("b", FILTER_ID, NULL);

	record_start(src);
	obs_source_restore_filters(NULL, array);
	obs_source_restore_filters(src, NULL);
	obs_source_restore_filters(NULL, NULL);
	assert_int_equal(null_lines, 3);
	record_expect("obs_source_restore_filters: Null 'source' parameter(free)|"
		      "obs_source_restore_filters: Null 'array' parameter(free)|"
		      "obs_source_restore_filters: Null 'source' parameter(free)");

	expect_chain(src, "a>src");
	assert_true(mutex_free(src));
	obs_data_array_release(array);
	expect_one_list_reference(src, a);
	drop(src);
}

static void reuses_matching_filters(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *c = add_filter(src, FILTER_ID, "c");
	obs_source_t *b = add_filter(src, FILTER_ID, "b");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	expect_chain(src, "a>b,b>c,c>src");
	/* The array's ids don't matter for a filter that is reused. */
	obs_data_array_t *array = backup_of("c", OTHER_FILTER_ID, "a", FILTER_ID, "b", FILTER_ID, NULL);

	record_start(src);
	obs_source_restore_filters(src, array);
	record_expect("");
	/* Nothing logs when every item is reused. */
	assert_int_equal(all_lines, 0);

	expect_chain(src, "c>a,a>b,b>src");
	assert_string_equal(obs_source_get_id(c), FILTER_ID);
	assert_true(mutex_free(src));
	obs_data_array_release(array);
	/* The old reference was released, the new one kept. */
	expect_one_list_reference(src, a);
	expect_one_list_reference(src, b);
	expect_one_list_reference(src, c);
	drop(src);
}

static void loads_unmatched_items_with_the_lock_free(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *b = add_filter(src, FILTER_ID, "b");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	expect_chain(src, "a>b,b>src");
	obs_data_array_t *array =
		backup_of("n1", FILTER_ID, "a", FILTER_ID, "n2", OTHER_FILTER_ID, "b", FILTER_ID, NULL);

	/* While n1 loads, the list is empty and a (reused next) has no parent
	 * and no target. While n2 loads, a still has neither: its target is
	 * set once n2 is loaded, its parent at the end. */
	record_start(src);
	watched_filter = a;
	obs_source_restore_filters(src, array);
	record_expect("load(n1,0,free,->-)|load(n2,0,free,->-)");

	expect_chain(src, "n1>a,a>n2,n2>b,b>src");
	obs_source_t *n1 = listed(src, "n1");
	obs_source_t *n2 = listed(src, "n2");
	assert_string_equal(obs_source_get_id(n1), FILTER_ID);
	assert_string_equal(obs_source_get_id(n2), OTHER_FILTER_ID);
	obs_data_array_release(array);
	expect_one_list_reference(src, a);
	expect_one_list_reference(src, b);
	expect_one_list_reference(src, n1);
	expect_one_list_reference(src, n2);
	drop(src);
}

static void targets_are_set_before_parents(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	obs_data_array_t *array = backup_of("a", FILTER_ID, "n1", FILTER_ID, "n2", FILTER_ID, NULL);

	/* While n2 loads, a targets n1 (set once n1 was loaded) but has no
	 * parent yet (#2172). */
	record_start(src);
	watched_filter = a;
	obs_source_restore_filters(src, array);
	record_expect("load(n1,0,free,->-)|load(n2,0,free,->n1)");

	expect_chain(src, "a>n1,n1>n2,n2>src");
	obs_data_array_release(array);
	expect_one_list_reference(src, a);
	drop(src);
}

static void releases_old_filters_in_old_order(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *z = add_filter(src, FILTER_ID, "z");
	obs_source_t *y = add_filter(src, FILTER_ID, "y");
	obs_source_t *x = add_filter(src, FILTER_ID, "x");
	watch(x);
	watch(z);
	expect_chain(src, "x>y,y>z,z>src");
	/* Only the list holds x and z. */
	obs_source_release(x);
	obs_source_release(z);
	obs_data_array_t *array = backup_of("y", FILTER_ID, NULL);

	/* x and z go away once the new list is in place and the lock free. */
	record_start(src);
	obs_source_restore_filters(src, array);
	record_expect("released(x,1,free)|released(z,1,free)");

	expect_chain(src, "y>src");
	obs_data_array_release(array);
	expect_one_list_reference(src, y);
	drop(src);
}

static void empty_array_drops_every_filter(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *b = add_filter(src, FILTER_ID, "b");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	obs_weak_source_t *weak_a = obs_source_get_weak_source(a);
	obs_weak_source_t *weak_b = obs_source_get_weak_source(b);
	obs_data_array_t *array = obs_data_array_create();

	record_start(src);
	obs_source_restore_filters(src, array);
	record_expect("");

	expect_chain(src, "");
	assert_null(obs_filter_get_parent(a));
	assert_null(obs_filter_get_target(a));
	assert_null(obs_filter_get_parent(b));
	assert_null(obs_filter_get_target(b));
	/* The list's references are gone: the caller's were the last. */
	obs_source_release(a);
	obs_source_release(b);
	assert_true(obs_weak_source_expired(weak_a));
	assert_true(obs_weak_source_expired(weak_b));
	obs_weak_source_release(weak_a);
	obs_weak_source_release(weak_b);

	/* An empty array on an empty list: still nothing. */
	record_start(src);
	obs_source_restore_filters(src, array);
	record_expect("");
	assert_int_equal(all_lines, 0);
	expect_chain(src, "");

	obs_data_array_release(array);
	drop(src);
}

static void duplicate_names_reuse_the_same_filter(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *b = add_filter(src, FILTER_ID, "b");
	obs_source_t *a = add_filter(src, FILTER_ID, "a");
	obs_data_array_t *array = backup_of("a", FILTER_ID, "a", OTHER_FILTER_ID, NULL);

	record_start(src);
	obs_source_restore_filters(src, array);
	record_expect("");

	/* a is listed twice; the last target written (the source) wins. */
	expect_chain(src, "a>src,a>src");
	obs_data_array_release(array);
	/* Two list references: the first remove leaves a listed. */
	obs_weak_source_t *weak = obs_source_get_weak_source(a);
	obs_source_release(a);
	obs_source_filter_remove(src, a);
	assert_false(obs_weak_source_expired(weak));
	assert_int_equal(obs_source_filter_count(src), 1);
	obs_source_filter_remove(src, a);
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
	/* b, not in the array, was dropped. */
	assert_null(obs_filter_get_parent(b));
	obs_source_release(b);
	drop(src);
}

static void unnamed_filter_never_matches(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *x = add_filter(src, FILTER_ID, "x");
	obs_source_t *unnamed = add_filter(src, FILTER_ID, NULL);
	assert_null(obs_source_get_name(unnamed));
	obs_weak_source_t *weak = obs_source_get_weak_source(unnamed);
	/* No filter can be named "" (a blank name gets a generated one), so
	 * an item named "" matches no current filter, the unnamed one
	 * included: it is loaded. */
	obs_data_array_t *array = backup_of("", FILTER_ID, "x", FILTER_ID, NULL);

	record_start(src);
	watched_filter = x;
	obs_source_restore_filters(src, array);
	record_expect("load(generated,0,free,->-)");

	assert_int_equal(obs_source_filter_count(src), 2);
	obs_source_t *loaded = listed(src, "x");
	assert_ptr_equal(loaded, x);
	obs_source_release(loaded);
	assert_null(obs_filter_get_parent(unnamed));
	assert_null(obs_filter_get_target(unnamed));
	obs_source_release(unnamed);
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
	obs_data_array_release(array);
	expect_one_list_reference(src, x);
	drop(src);
}

static void no_task(void *param)
{
	(void)param;
}

static int teardown(void **state)
{
	(void)state;
	recording = false;
	watched = NULL;
	watched_filter = NULL;
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
		signal_handler_t *handler = obs_get_signal_handler();
		for (size_t i = 0; i < SIGNAL_COUNT(global_signals); i++)
			signal_handler_connect(handler, global_signals[i], on_filter_signal, (void *)global_signals[i]);
	} else {
		fprintf(stderr, "obs_startup failed (no display?), skipping\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		signal_handler_t *handler = obs_get_signal_handler();
		for (size_t i = 0; i < SIGNAL_COUNT(global_signals); i++)
			signal_handler_disconnect(handler, global_signals[i], on_filter_signal,
						  (void *)global_signals[i]);
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(null_arguments, teardown),
		cmocka_unit_test_teardown(reuses_matching_filters, teardown),
		cmocka_unit_test_teardown(loads_unmatched_items_with_the_lock_free, teardown),
		cmocka_unit_test_teardown(targets_are_set_before_parents, teardown),
		cmocka_unit_test_teardown(releases_old_filters_in_old_order, teardown),
		cmocka_unit_test_teardown(empty_array_drops_every_filter, teardown),
		cmocka_unit_test_teardown(duplicate_names_reuse_the_same_filter, teardown),
		cmocka_unit_test_teardown(unnamed_filter_never_matches, teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

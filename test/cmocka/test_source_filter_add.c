/* Characterization of obs_source_filter_add() (#5799), against the hybrid
 * libobs and the C reference through the real core.
 *
 * - A NULL source or filter logs the validity check's line once (the source
 *   first) and adds, signals and references nothing.
 * - A filter already on the source's list logs the warning "Tried to add a
 *   filter that was already present on the source" with filter_mutex held
 *   and changes nothing: no signal, no callback, no new reference. (A filter
 *   on another source's list is not added here: the C reference takes it,
 *   hybrid libobs refuses it with the same warning, an intentional
 *   difference of the filter chain rules, so neither is checked.)
 * - A filter whose output flags don't fit the source's (an audio filter on a
 *   video source) is silently refused, unless the source's type is missing
 *   (it owns its info id), which skips the check.
 * - A filter that is being destroyed (no reference can be taken) logs the
 *   validity check's line for 'filter' and adds nothing.
 * - Otherwise the filter goes to the front of the list, holding a new
 *   reference, its parent is the source and its target the old first filter
 *   (the source on an empty list). Then, with filter_mutex released, the
 *   global "source_filter_add" signal and the source's own "filter_add"
 *   signal fire (both with the source and the filter, the list already
 *   holding it), the debug line "- filter '<name>' (<id>) added to source
 *   '<name>'" is logged, and the filter type's filter_add callback runs with
 *   the parent when the type has one.
 *
 * Whether filter_mutex is free when a signal, the callback or a log line runs
 * is checked from a second thread (the mutex is recursive, so the calling
 * thread could always take it): obs_source_filter_get_index() takes the
 * mutex there.
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

#define BOX_ID "filter_add_box"
#define MISSING_ID "filter_add_missing_type"
#define FILTER_ID "filter_add_filter"
#define PLAIN_FILTER_ID "filter_add_plain_filter"
#define AUDIO_FILTER_ID "filter_add_audio_filter"
#define READD_FILTER_ID "filter_add_readd_filter"

static bool obs_up;

/* What happened, in order, while recording: signals, callbacks and the log
 * lines about adding filters, separated by '|'. */
static bool recording;
static struct dstr events;
static int null_lines;

/* The source whose filter_mutex the log handler probes. */
static obs_source_t *probe_source;

/* The source the readd filter type adds its dying parent to. */
static obs_source_t *readd_target;

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
	if (!probe_source)
		return "-";
	return mutex_free(probe_source) ? "free" : "held";
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
		if (lvl == LOG_DEBUG && strstr(line, "obs_source_filter_add")) {
			struct dstr text = {0};
			dstr_printf(&text, "%s(%s)", line, lock_state());
			record(text.array);
			dstr_free(&text);
			null_lines++;
		} else if (lvl == LOG_WARNING && strstr(line, "already present")) {
			struct dstr text = {0};
			dstr_printf(&text, "%s(%s)", line, lock_state());
			record(text.array);
			dstr_free(&text);
		} else if (lvl == LOG_DEBUG && strstr(line, "added to source")) {
			record(line);
		}
	}
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static void record_start(obs_source_t *probe)
{
	dstr_free(&events);
	null_lines = 0;
	probe_source = probe;
	recording = true;
}

static void record_expect(const char *expected)
{
	recording = false;
	probe_source = NULL;
	join_stuck_probe();
	assert_string_equal(events.array ? events.array : "", expected);
	dstr_free(&events);
}

static const char *name_of(obs_source_t *source)
{
	return source ? obs_source_get_name(source) : "-";
}

/* "<tag>(<source>,<filter>,<filter's parent>,<filter's target>,<count>,<free|held>)" */
static void on_add(const char *tag, calldata_t *cd)
{
	obs_source_t *source = calldata_ptr(cd, "source");
	obs_source_t *filter = calldata_ptr(cd, "filter");
	struct dstr text = {0};
	dstr_printf(&text, "%s(%s,%s,%s,%s,%d,%s)", tag, name_of(source), name_of(filter),
		    name_of(obs_filter_get_parent(filter)), name_of(obs_filter_get_target(filter)),
		    (int)obs_source_filter_count(source), mutex_free(source) ? "free" : "held");
	record(text.array);
	dstr_free(&text);
}

static void on_global_add(void *data, calldata_t *cd)
{
	(void)data;
	on_add("global", cd);
}

static void on_source_add(void *data, calldata_t *cd)
{
	(void)data;
	on_add("source", cd);
}

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static const char *filter_name(void *type_data)
{
	(void)type_data;
	return "add test filter";
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
static void filter_added(void *data, obs_source_t *parent)
{
	obs_source_t *filter = data;
	struct dstr text = {0};
	dstr_printf(&text, "callback(%s,%s,%s,%s,%s)", name_of(filter), name_of(parent),
		    name_of(obs_filter_get_parent(filter)), name_of(obs_filter_get_target(filter)),
		    parent && mutex_free(parent) ? "free" : "held");
	record(text.array);
	dstr_free(&text);
}

/* Runs while parent is being destroyed (its last reference is gone): adds
 * the dying parent to readd_target. */
static void readd_parent(void *data, obs_source_t *parent)
{
	(void)data;
	if (readd_target)
		obs_source_filter_add(readd_target, parent);
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
		.filter_add = filter_added,
	};
	struct obs_source_info plain_filter = {
		.id = PLAIN_FILTER_ID,
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
		.filter_add = filter_added,
	};
	struct obs_source_info readd_filter = {
		.id = READD_FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = filter_name,
		.create = data_create,
		.destroy = data_destroy,
		.filter_remove = readd_parent,
	};
	obs_register_source(&box);
	obs_register_source(&filter);
	obs_register_source(&plain_filter);
	obs_register_source(&audio_filter);
	obs_register_source(&readd_filter);
}

static obs_source_t *source_of(const char *id, const char *name)
{
	obs_source_t *source = obs_source_create(id, name, NULL, NULL);
	assert_non_null(source);
	signal_handler_connect(obs_source_get_signal_handler(source), "filter_add", on_source_add, NULL);
	return source;
}

static obs_source_t *box(const char *name)
{
	return source_of(BOX_ID, name);
}

/* A new filter, with a reference the caller owns. */
static obs_source_t *new_filter(const char *id, const char *name)
{
	obs_source_t *filter = obs_source_create_private(id, name, NULL);
	assert_non_null(filter);
	return filter;
}

static void drop(obs_source_t *source)
{
	signal_handler_disconnect(obs_source_get_signal_handler(source), "filter_add", on_source_add, NULL);
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
	obs_source_t *a = new_filter(FILTER_ID, "a");
	obs_weak_source_t *weak = obs_source_get_weak_source(a);

	record_start(NULL);
	obs_source_filter_add(NULL, a);
	obs_source_filter_add(src, NULL);
	obs_source_filter_add(NULL, NULL);
	assert_int_equal(null_lines, 3);
	record_expect("obs_source_filter_add: Null 'source' parameter(-)|"
		      "obs_source_filter_add: Null 'filter' parameter(-)|"
		      "obs_source_filter_add: Null 'source' parameter(-)");

	expect_chain(src, "");
	assert_null(obs_filter_get_parent(a));
	/* Nothing was referenced: the caller's reference is the last one. */
	obs_source_release(a);
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
	drop(src);
}

static void add_builds_the_chain_from_the_front(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *a = new_filter(FILTER_ID, "a");
	obs_source_t *b = new_filter(PLAIN_FILTER_ID, "b");
	obs_source_t *c = new_filter(FILTER_ID, "c");
	obs_weak_source_t *weak = obs_source_get_weak_source(a);

	record_start(NULL);
	obs_source_filter_add(src, a);
	record_expect("global(src,a,src,src,1,free)|source(src,a,src,src,1,free)|"
		      "- filter 'a' (" FILTER_ID ") added to source 'src'|"
		      "callback(a,src,src,src,free)");
	expect_chain(src, "a>src");

	/* A type without a filter_add callback: everything but the call. */
	record_start(NULL);
	obs_source_filter_add(src, b);
	record_expect("global(src,b,src,a,2,free)|source(src,b,src,a,2,free)|"
		      "- filter 'b' (" PLAIN_FILTER_ID ") added to source 'src'");
	expect_chain(src, "b>a,a>src");

	record_start(NULL);
	obs_source_filter_add(src, c);
	record_expect("global(src,c,src,b,3,free)|source(src,c,src,b,3,free)|"
		      "- filter 'c' (" FILTER_ID ") added to source 'src'|"
		      "callback(c,src,src,b,free)");
	expect_chain(src, "c>b,b>a,a>src");

	/* The list holds its own reference. */
	obs_source_release(a);
	assert_false(obs_weak_source_expired(weak));
	assert_ptr_equal(obs_filter_get_parent(a), src);
	obs_source_filter_remove(src, a);
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);

	obs_source_release(b);
	obs_source_release(c);
	drop(src);
}

static void listed_filter_warns(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *a = new_filter(FILTER_ID, "a");
	obs_source_t *b = new_filter(FILTER_ID, "b");
	obs_source_filter_add(src, a);
	obs_source_filter_add(src, b);
	obs_weak_source_t *weak = obs_source_get_weak_source(b);

	record_start(src);
	obs_source_filter_add(src, a);
	record_expect("Tried to add a filter that was already present on the source(held)");
	record_start(src);
	obs_source_filter_add(src, b);
	record_expect("Tried to add a filter that was already present on the source(held)");

	expect_chain(src, "b>a,a>src");

	/* No new reference: the list's is the last one after the caller's. */
	obs_source_release(b);
	obs_source_filter_remove(src, b);
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);

	obs_source_release(a);
	drop(src);
}

static void incompatible_filter_is_refused_silently(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *loose = source_of(MISSING_ID, "loose");
	obs_source_t *aud = new_filter(AUDIO_FILTER_ID, "aud");
	obs_weak_source_t *weak = obs_source_get_weak_source(aud);

	/* An audio filter does not fit a video source. */
	record_start(src);
	obs_source_filter_add(src, aud);
	record_expect("");
	expect_chain(src, "");
	assert_null(obs_filter_get_parent(aud));
	assert_true(mutex_free(src));

	/* A source whose type is missing takes any filter. */
	record_start(NULL);
	obs_source_filter_add(loose, aud);
	record_expect("global(loose,aud,loose,loose,1,free)|source(loose,aud,loose,loose,1,free)|"
		      "- filter 'aud' (" AUDIO_FILTER_ID ") added to source 'loose'|"
		      "callback(aud,loose,loose,loose,free)");
	expect_chain(loose, "aud>loose");

	obs_source_release(aud);
	assert_false(obs_weak_source_expired(weak));
	drop(loose);
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
	drop(src);
}

static void dying_filter_is_not_added(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	obs_source_t *src = box("src");
	obs_source_t *dying = new_filter(PLAIN_FILTER_ID, "dying");
	obs_source_t *child = new_filter(READD_FILTER_ID, "child");
	obs_source_filter_add(dying, child);
	obs_source_release(child);

	/* Releasing the last reference destroys dying, which removes child;
	 * child's filter_remove callback adds dying to src. */
	readd_target = src;
	record_start(NULL);
	obs_source_release(dying);
	readd_target = NULL;
	record_expect("obs_source_filter_add: Null 'filter' parameter(-)");
	assert_int_equal(null_lines, 1);

	/* Whether filter_mutex is unlocked afterwards is not checked here:
	 * the C reference returns with it still locked (#2981), the Rust
	 * port unlocks it. Only this thread touches src from now on (the
	 * mutex is recursive). */
	expect_chain(src, "");
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
	probe_source = NULL;
	readd_target = NULL;
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
		signal_handler_connect(obs_get_signal_handler(), "source_filter_add", on_global_add, NULL);
	} else {
		fprintf(stderr, "obs_startup failed (no display?), skipping\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		signal_handler_disconnect(obs_get_signal_handler(), "source_filter_add", on_global_add, NULL);
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(null_arguments, teardown),
		cmocka_unit_test_teardown(add_builds_the_chain_from_the_front, teardown),
		cmocka_unit_test_teardown(listed_filter_warns, teardown),
		cmocka_unit_test_teardown(incompatible_filter_is_refused_silently, teardown),
		cmocka_unit_test_teardown(dying_filter_is_not_added, teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

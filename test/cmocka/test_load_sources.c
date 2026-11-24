/* Characterization of obs_load_sources() (#5571), the scene-collection load
 * every frontend runs at startup and on every collection switch, driven
 * through the public API on a live core (obs_startup, obs_reset_video with the
 * OpenGL module).
 *
 * What it pins:
 *
 *   - an empty array creates no source and never calls cb;
 *   - pass 1 creates every entry, in array order, before pass 2 loads any of
 *     them: the global "source_create" signal fires for every entry before
 *     the first source's load callback runs, and each load callback sees
 *     every source of the array already existing (obs_get_source_by_name);
 *   - pass 2 runs obs_source_load2() (the type's load callback) and then
 *     cb(private_data, source), once per source and in array order, with
 *     private_data unchanged; cb may be NULL (the sources still load);
 *   - an entry whose type is not registered (a missing plugin) is still
 *     created and still reaches cb, but gets no load (it has no data);
 *   - a transition entry goes through obs_transition_load() before its own
 *     load callback and its cb: its saved alignment, scale type and size are
 *     applied, and its saved "transition_source_a" is found even though that
 *     source comes later in the array;
 *   - references: the call holds each source while it runs (a load callback
 *     and cb see it alive) and drops its own reference at the end: a source
 *     nobody else took a reference to is destroyed after the call, and one cb
 *     kept a reference to is destroyed by obs_source_remove plus that one
 *     release. The core's source list holds no reference of its own.
 *
 * obs_load_source() never returns NULL through the public API (an unknown
 * type still makes a source, and a failed creation would be dereferenced in
 * obs_load_source_type() before obs_load_sources() saw it), so the
 * NULL-entry branch of pass 2 is not reachable here.
 *
 * The video thread takes a short reference on every source each tick, so a
 * released source is waited for (bounded) through a weak reference rather
 * than counted.
 *
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>

#define INPUT_ID "test_load_sources_input"
#define TRANSITION_ID "test_load_sources_transition"
#define MISSING_ID "test_load_sources_missing_type"
#define TIMEOUT_MS 10000
#define MAX_EVENTS 32
#define MAX_SOURCES 8

static bool obs_up;

/* ------------------------------------------------------------------------- */
/* recording                                                                 */

struct record {
	/* "create:<name>", "load:<name>", "cb:<name>", in the order they happen */
	char events[MAX_EVENTS][64];
	size_t event_count;

	/* names every load callback expects to exist already */
	const char *const *names;
	size_t name_count;
	bool all_present_at_load;
	size_t loads;

	/* the transition's state as its own load callback saw it */
	bool transition_loaded;
	uint32_t transition_alignment;
	enum obs_transition_scale_type transition_scale_type;
	uint32_t transition_cx;
	uint32_t transition_cy;
	char transition_source_a[64];

	/* cb */
	void *private_data;
	bool private_data_ok;
	size_t cb_count;
	obs_source_t *cb_sources[MAX_SOURCES];
	/* cb keeps a reference to the source at cb index i when bit i is set */
	unsigned keep_mask;
	obs_source_t *kept[MAX_SOURCES];

	/* weak references to every source the "source_create" signal reports */
	obs_weak_source_t *created[MAX_SOURCES];
	size_t created_count;
};

static struct record *current;

static void add_event(const char *kind, obs_source_t *source)
{
	if (!current)
		return;
	if (current->event_count < MAX_EVENTS)
		snprintf(current->events[current->event_count], sizeof(current->events[0]), "%s:%s", kind,
			 obs_source_get_name(source));
	current->event_count++;
}

static void on_source_create(void *data, calldata_t *cd)
{
	(void)data;
	obs_source_t *source = calldata_ptr(cd, "source");
	if (!current || !source)
		return;
	add_event("create", source);
	if (current->created_count < MAX_SOURCES)
		current->created[current->created_count++] = obs_source_get_weak_source(source);
}

static void record_cb(void *private_data, obs_source_t *source)
{
	struct record *r = current;
	assert_non_null(r);
	if (private_data != r->private_data)
		r->private_data_ok = false;
	add_event("cb", source);
	size_t index = r->cb_count++;
	if (index < MAX_SOURCES) {
		r->cb_sources[index] = source;
		if (r->keep_mask & (1u << index))
			r->kept[index] = obs_source_get_ref(source);
	}
}

static void begin(struct record *r, const char *const *names, size_t name_count)
{
	memset(r, 0, sizeof(*r));
	r->names = names;
	r->name_count = name_count;
	r->all_present_at_load = true;
	r->private_data_ok = true;
	r->private_data = r;
	current = r;
	signal_handler_connect(obs_get_signal_handler(), "source_create", on_source_create, NULL);
}

static void end(struct record *r)
{
	(void)r;
	signal_handler_disconnect(obs_get_signal_handler(), "source_create", on_source_create, NULL);
	current = NULL;
}

static void assert_events(const struct record *r, const char *const *expected, size_t count)
{
	for (size_t i = 0; i < r->event_count && i < MAX_EVENTS; i++)
		fprintf(stderr, "event %zu: %s\n", i, r->events[i]);
	assert_int_equal(r->event_count, count);
	for (size_t i = 0; i < count; i++)
		assert_string_equal(r->events[i], expected[i]);
}

/* ------------------------------------------------------------------------- */
/* source types                                                              */

struct type_data {
	obs_source_t *source;
};

static const char *type_name(void *type_data)
{
	(void)type_data;
	return "load sources test";
}

static void *type_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	struct type_data *data = bzalloc(sizeof(*data));
	data->source = source;
	return data;
}

static void type_destroy(void *data)
{
	bfree(data);
}

static uint32_t type_size(void *data)
{
	(void)data;
	return 16;
}

static void type_load(void *data, obs_data_t *settings)
{
	(void)settings;
	struct type_data *d = data;
	struct record *r = current;
	if (!r)
		return;
	r->loads++;
	add_event("load", d->source);

	for (size_t i = 0; i < r->name_count; i++) {
		obs_source_t *other = obs_get_source_by_name(r->names[i]);
		if (!other)
			r->all_present_at_load = false;
		obs_source_release(other);
	}

	if (obs_source_get_type(d->source) == OBS_SOURCE_TYPE_TRANSITION) {
		r->transition_loaded = true;
		r->transition_alignment = obs_transition_get_alignment(d->source);
		r->transition_scale_type = obs_transition_get_scale_type(d->source);
		obs_transition_get_size(d->source, &r->transition_cx, &r->transition_cy);
		obs_source_t *a = obs_transition_get_source(d->source, OBS_TRANSITION_SOURCE_A);
		snprintf(r->transition_source_a, sizeof(r->transition_source_a), "%s", a ? obs_source_get_name(a) : "");
		obs_source_release(a);
	}
}

static void transition_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
}

static bool transition_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
				    size_t channels, size_t sample_rate)
{
	(void)data;
	(void)ts_out;
	(void)audio;
	(void)mixers;
	(void)channels;
	(void)sample_rate;
	return false;
}

static void register_types(void)
{
	struct obs_source_info input = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_width = type_size,
		.get_height = type_size,
		.get_name = type_name,
		.create = type_create,
		.destroy = type_destroy,
		.load = type_load,
	};
	obs_register_source(&input);

	struct obs_source_info transition = {
		.id = TRANSITION_ID,
		.type = OBS_SOURCE_TYPE_TRANSITION,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = type_name,
		.create = type_create,
		.destroy = type_destroy,
		.load = type_load,
		.video_render = transition_render,
		.audio_render = transition_audio_render,
	};
	obs_register_source(&transition);
}

/* ------------------------------------------------------------------------- */
/* helpers                                                                   */

static void push_entry(obs_data_array_t *array, const char *id, const char *name)
{
	obs_data_t *entry = obs_data_create();
	obs_data_set_string(entry, "id", id);
	obs_data_set_string(entry, "name", name);
	obs_data_array_push_back(array, entry);
	obs_data_release(entry);
}

/* Waits (bounded) until the source behind a weak reference is destroyed: the
 * video thread may still hold a tick reference for a moment. */
static void wait_destroyed(obs_weak_source_t *weak)
{
	uint64_t deadline = os_gettime_ns() + (uint64_t)TIMEOUT_MS * 1000000ULL;
	for (;;) {
		obs_source_t *alive = obs_weak_source_get_source(weak);
		if (!alive)
			break;
		obs_source_release(alive);
		if (os_gettime_ns() > deadline)
			fail_msg("a released source was not destroyed within %d ms", TIMEOUT_MS);
		os_sleep_ms(5);
	}
	obs_wait_for_destroy_queue();
}

static bool weak_alive(obs_weak_source_t *weak, obs_source_t *expected)
{
	obs_source_t *alive = obs_weak_source_get_source(weak);
	bool ok = alive && (!expected || alive == expected);
	obs_source_release(alive);
	return ok;
}

/* Removes a source the caller holds one reference to, drops that reference
 * and checks that this destroys it. */
static void remove_release_and_wait(obs_source_t *source)
{
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	obs_source_remove(source);
	obs_source_release(source);
	wait_destroyed(weak);
	assert_false(weak_alive(weak, NULL));
	obs_weak_source_release(weak);
}

static void release_created(struct record *r)
{
	for (size_t i = 0; i < r->created_count; i++)
		obs_weak_source_release(r->created[i]);
	r->created_count = 0;
}

static void assert_core_has_no_sources(void)
{
	obs_data_array_t *array = obs_save_sources();
	assert_non_null(array);
	assert_int_equal(obs_data_array_count(array), 0);
	obs_data_array_release(array);
}

/* ------------------------------------------------------------------------- */
/* cases                                                                     */

#define SKIP_WITHOUT_CORE()                                     \
	do {                                                    \
		if (!obs_up) {                                  \
			fprintf(stderr, "skipped: no live core\n"); \
			skip();                                 \
		}                                               \
	} while (0)

static void empty_array_creates_nothing_and_never_calls_back(void **state)
{
	(void)state;
	SKIP_WITHOUT_CORE();

	obs_data_array_t *array = obs_data_array_create();
	struct record r;
	begin(&r, NULL, 0);
	obs_load_sources(array, record_cb, r.private_data);
	obs_load_sources(array, NULL, NULL);
	end(&r);

	assert_int_equal(r.event_count, 0);
	assert_int_equal(r.cb_count, 0);
	assert_int_equal(r.loads, 0);
	assert_int_equal(r.created_count, 0);
	assert_int_equal(obs_data_array_count(array), 0);
	obs_data_array_release(array);
	assert_core_has_no_sources();
}

static void creates_all_then_loads_and_calls_back_in_order(void **state)
{
	(void)state;
	SKIP_WITHOUT_CORE();

	static const char *const names[] = {
		"load a", "load missing", "load b", "load transition", "load c",
	};
	const size_t n = sizeof(names) / sizeof(names[0]);

	obs_data_array_t *array = obs_data_array_create();
	push_entry(array, INPUT_ID, names[0]);
	push_entry(array, MISSING_ID, names[1]);
	push_entry(array, INPUT_ID, names[2]);
	push_entry(array, TRANSITION_ID, names[3]);
	push_entry(array, INPUT_ID, names[4]);

	/* The transition's saved state names a source later in the array. */
	obs_data_t *tr_entry = obs_data_array_item(array, 3);
	obs_data_set_string(tr_entry, "transition_source_a", names[4]);
	obs_data_set_int(tr_entry, "transition_alignment", OBS_ALIGN_LEFT | OBS_ALIGN_TOP);
	obs_data_set_int(tr_entry, "transition_scale_type", OBS_TRANSITION_SCALE_STRETCH);
	obs_data_set_int(tr_entry, "transition_cx", 32);
	obs_data_set_int(tr_entry, "transition_cy", 24);
	obs_data_release(tr_entry);

	struct record r;
	begin(&r, names, n);
	r.keep_mask = (1u << n) - 1;
	obs_load_sources(array, record_cb, r.private_data);
	end(&r);

	static const char *const expected[] = {
		"create:load a",
		"create:load missing",
		"create:load b",
		"create:load transition",
		"create:load c",
		"load:load a",
		"cb:load a",
		"cb:load missing",
		"load:load b",
		"cb:load b",
		"load:load transition",
		"cb:load transition",
		"load:load c",
		"cb:load c",
	};
	assert_events(&r, expected, sizeof(expected) / sizeof(expected[0]));
	assert_true(r.all_present_at_load);
	assert_int_equal(r.loads, 4);
	assert_true(r.private_data_ok);
	assert_int_equal(r.cb_count, n);
	assert_int_equal(r.created_count, n);

	/* cb saw the sources pass 1 created, in array order. */
	for (size_t i = 0; i < n; i++) {
		assert_non_null(r.kept[i]);
		assert_ptr_equal(r.cb_sources[i], r.kept[i]);
		assert_string_equal(obs_source_get_name(r.kept[i]), names[i]);
		assert_true(weak_alive(r.created[i], r.kept[i]));
	}

	/* The entry with no registered type was created anyway. */
	obs_source_t *missing = r.kept[1];
	assert_string_equal(obs_source_get_id(missing), MISSING_ID);

	/* The transition's saved state was applied before its load callback. */
	obs_source_t *tr = r.kept[3];
	assert_int_equal(obs_source_get_type(tr), OBS_SOURCE_TYPE_TRANSITION);
	assert_true(r.transition_loaded);
	assert_int_equal(r.transition_alignment, OBS_ALIGN_LEFT | OBS_ALIGN_TOP);
	assert_int_equal(r.transition_scale_type, OBS_TRANSITION_SCALE_STRETCH);
	assert_int_equal(r.transition_cx, 32);
	assert_int_equal(r.transition_cy, 24);
	assert_string_equal(r.transition_source_a, names[4]);
	obs_source_t *a = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_A);
	assert_ptr_equal(a, r.kept[4]);
	obs_source_release(a);
	assert_int_equal(obs_transition_get_alignment(tr), OBS_ALIGN_LEFT | OBS_ALIGN_TOP);

	obs_transition_clear(tr);
	release_created(&r);
	for (size_t i = 0; i < n; i++)
		remove_release_and_wait(r.kept[i]);
	obs_data_array_release(array);
	assert_core_has_no_sources();
}

static void null_cb_still_loads(void **state)
{
	(void)state;
	SKIP_WITHOUT_CORE();

	static const char *const names[] = {"null cb x", "null cb y"};
	obs_data_array_t *array = obs_data_array_create();
	push_entry(array, INPUT_ID, names[0]);
	push_entry(array, INPUT_ID, names[1]);

	struct record r;
	begin(&r, names, 2);
	obs_load_sources(array, NULL, NULL);
	end(&r);

	static const char *const expected[] = {
		"create:null cb x",
		"create:null cb y",
		"load:null cb x",
		"load:null cb y",
	};
	assert_events(&r, expected, 4);
	assert_true(r.all_present_at_load);
	assert_int_equal(r.cb_count, 0);
	assert_int_equal(r.created_count, 2);

	/* Nobody took a reference, so the call's own release destroys them. */
	for (size_t i = 0; i < r.created_count; i++)
		wait_destroyed(r.created[i]);
	release_created(&r);
	obs_data_array_release(array);
	assert_core_has_no_sources();
}

static void call_leaves_no_reference_behind(void **state)
{
	(void)state;
	SKIP_WITHOUT_CORE();

	static const char *const names[] = {"ref kept", "ref dropped", "ref kept too"};
	obs_data_array_t *array = obs_data_array_create();
	for (size_t i = 0; i < 3; i++)
		push_entry(array, INPUT_ID, names[i]);

	struct record r;
	begin(&r, names, 3);
	r.keep_mask = 0x5u;
	obs_load_sources(array, record_cb, r.private_data);
	end(&r);

	assert_true(r.private_data_ok);
	assert_int_equal(r.cb_count, 3);
	assert_int_equal(r.loads, 3);
	assert_int_equal(r.created_count, 3);

	/* The source cb did not keep is gone once the call returns. */
	assert_null(r.kept[1]);
	wait_destroyed(r.created[1]);
	assert_false(weak_alive(r.created[1], NULL));

	/* The kept ones live while cb's reference is held... */
	obs_wait_for_destroy_queue();
	assert_true(weak_alive(r.created[0], r.kept[0]));
	assert_true(weak_alive(r.created[2], r.kept[2]));
	obs_source_t *found = obs_get_source_by_name(names[0]);
	assert_ptr_equal(found, r.kept[0]);
	obs_source_release(found);

	/* ...and that one reference is all that holds them. */
	remove_release_and_wait(r.kept[0]);
	remove_release_and_wait(r.kept[2]);
	assert_false(weak_alive(r.created[0], NULL));
	assert_false(weak_alive(r.created[2], NULL));

	release_created(&r);
	obs_data_array_release(array);
	assert_core_has_no_sources();
}

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
{
	(void)state;
	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "skipped: no display\n");
		return 0;
	}
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 16,
		.base_height = 16,
		.output_width = 16,
		.output_height = 16,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_POINT,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}

	register_types();
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_wait_for_destroy_queue();
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		/* first: nothing has been created yet */
		cmocka_unit_test(empty_array_creates_nothing_and_never_calls_back),
		cmocka_unit_test(creates_all_then_loads_and_calls_back_in_order),
		cmocka_unit_test(null_cb_still_loads),
		cmocka_unit_test(call_leaves_no_reference_behind),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

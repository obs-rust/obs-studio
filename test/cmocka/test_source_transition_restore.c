/*
 * Characterization of obs_transition_load() in libobs/obs-source-transition.c
 * (obs-rust/obs-studio#3581), before it moves to its own file and is ported.
 *
 * obs_transition_load() is not exported: obs_load_sources() calls it for
 * each transition of a scene collection's source array, after it has
 * created every source of the array. test_transition_scale_type pins the
 * loaded scale type through the drawn matrix, test_transition_load_release
 * the release of slot A's old source while the transition is shown, and
 * test_transition_load_steps the rules it calls. These cases pin, through
 * the real libobs:
 * - each field it writes from the saved data, read back through the
 *   getters and obs_save_source(): a key the data does not hold writes 0
 *   (so a missing alignment is OBS_ALIGN_CENTER, not a new transition's
 *   left and top), a scale type outside the enum is MAX_ONLY, and the
 *   alignment, mode and size are cast as they are (characterized, not
 *   endorsed);
 * - the actual size, recalculated from the loaded child at once;
 * - the three outcomes of the lookup of "transition_source_a": a found
 *   source goes into slot A as an active child (a shown transition shows
 *   it, and its clear takes it back); a missing one, or the empty name of
 *   a transition saved with no child, logs that it was not found and
 *   leaves slot A empty (characterized, not endorsed); the transition
 *   itself is refused with a log, and its reference given back;
 * - the source slot A held before the load is released, whatever the
 *   lookup found.
 *
 * Transitions take core video for their render targets, so the test only
 * builds on Linux and runs under xvfb-run; without a display the cases
 * skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/platform.h>

#define CHILD_ID "test_tr_restore_child"
#define TRANSITION_ID "test_tr_restore_transition"
#define CHILD_CX 16
#define CHILD_CY 24

static bool obs_up;

static const char *child_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "restore child";
}

static void *child_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void child_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static uint32_t child_width(void *data)
{
	UNUSED_PARAMETER(data);
	return CHILD_CX;
}

static uint32_t child_height(void *data)
{
	UNUSED_PARAMETER(data);
	return CHILD_CY;
}

static void child_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(effect);
}

static struct obs_source_info child_info = {
	.id = CHILD_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
	.get_name = child_name,
	.create = child_create,
	.destroy = child_destroy,
	.get_width = child_width,
	.get_height = child_height,
	.video_render = child_render,
};

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "restore transition";
}

static void *tr_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void tr_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static float mix_a(void *data, float t)
{
	UNUSED_PARAMETER(data);
	return 1.0f - t;
}

static float mix_b(void *data, float t)
{
	UNUSED_PARAMETER(data);
	return t;
}

static bool tr_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
			    size_t channels, size_t sample_rate)
{
	return obs_transition_audio_render(data, ts_out, audio, mixers, channels, sample_rate, mix_a, mix_b);
}

static struct obs_source_info transition_info = {
	.id = TRANSITION_ID,
	.type = OBS_SOURCE_TYPE_TRANSITION,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = tr_name,
	.create = tr_create,
	.destroy = tr_destroy,
	.audio_render = tr_audio_render,
};

/* The warnings logged since the case began, one per line. */
static char warnings[4096];
static log_handler_t prev_handler;
static void *prev_param;

static void record_log(int lvl, const char *msg, va_list args, void *p)
{
	UNUSED_PARAMETER(p);
	va_list copy;
	va_copy(copy, args);
	if (lvl == LOG_WARNING) {
		size_t len = strlen(warnings);
		vsnprintf(warnings + len, sizeof(warnings) - len, msg, copy);
		len = strlen(warnings);
		if (len + 1 < sizeof(warnings)) {
			warnings[len] = '\n';
			warnings[len + 1] = '\0';
		}
	}
	va_end(copy);
	if (prev_handler)
		prev_handler(lvl, msg, args, prev_param);
}

/* The sources of the running case, released by case_teardown if a failed
 * assertion ended the case first. */
static struct {
	obs_source_t *tr;
	obs_source_t *child;
	obs_source_t *old;
	obs_weak_source_t *weak_tr;
	obs_weak_source_t *weak_child;
	obs_weak_source_t *weak_old;
	obs_data_t *saved;
	bool shown;
} live;

static int case_setup(void **state)
{
	UNUSED_PARAMETER(state);
	warnings[0] = '\0';
	return 0;
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (live.shown)
		obs_source_dec_showing(live.tr);
	if (live.tr)
		obs_transition_clear(live.tr);
	obs_source_release(live.tr);
	obs_source_release(live.child);
	obs_source_release(live.old);
	obs_weak_source_release(live.weak_tr);
	obs_weak_source_release(live.weak_child);
	obs_weak_source_release(live.weak_old);
	obs_data_release(live.saved);
	memset(&live, 0, sizeof(live));
	return 0;
}

static void skip_without_core(void)
{
	if (!obs_up)
		skip();
}

static bool alive(obs_weak_source_t *weak)
{
	obs_source_t *s = obs_weak_source_get_source(weak);
	obs_source_release(s);
	return s != NULL;
}

/* Wait until the source of `weak` has no reference left. The graphics
 * thread holds a reference on every source while it ticks them, so the
 * last one can go a moment after the test's release. */
static bool gone(obs_weak_source_t *weak)
{
	uint64_t deadline = os_gettime_ns() + 5000000000ULL;
	while (alive(weak) && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	return !alive(weak);
}

/* What slot `target` of `tr` holds, without keeping a reference. */
static obs_source_t *slot(obs_source_t *tr, enum obs_transition_target target)
{
	obs_source_t *s = obs_transition_get_source(tr, target);
	obs_source_release(s);
	return s;
}

static obs_data_t *child_data(const char *name)
{
	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "id", CHILD_ID);
	obs_data_set_string(data, "name", name);
	return data;
}

static obs_data_t *transition_data(const char *name)
{
	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "id", TRANSITION_ID);
	obs_data_set_string(data, "name", name);
	return data;
}

/* What obs_load_sources() hands each callback: the transition is kept,
 * and, for the source named `set_from`, the transition is set to `set_to`
 * before its load runs. */
static struct {
	const char *set_from;
	obs_source_t *set_to;
	const char *tr_name;
} load_plan;

static void on_loaded(void *param, obs_source_t *source)
{
	UNUSED_PARAMETER(param);
	const char *name = obs_source_get_name(source);
	if (obs_source_get_type(source) == OBS_SOURCE_TYPE_TRANSITION) {
		assert_null(live.tr);
		live.tr = obs_source_get_ref(source);
		live.weak_tr = obs_source_get_weak_source(source);
	} else if (load_plan.set_from && strcmp(name, load_plan.set_from) == 0) {
		obs_source_t *tr = obs_get_source_by_name(load_plan.tr_name);
		assert_non_null(tr);
		obs_transition_set(tr, load_plan.set_to);
		obs_source_release(tr);
	}
}

/* Load `items` (released here) as a scene collection's source array. */
static obs_source_t *load(obs_data_t **items, size_t count)
{
	obs_data_array_t *array = obs_data_array_create();
	for (size_t i = 0; i < count; i++) {
		obs_data_array_push_back(array, items[i]);
		obs_data_release(items[i]);
	}
	obs_load_sources(array, on_loaded, NULL);
	obs_data_array_release(array);
	assert_non_null(live.tr);
	return live.tr;
}

/* Keep a reference on the loaded child named `name`. */
static obs_source_t *keep_child(const char *name)
{
	live.child = obs_get_source_by_name(name);
	assert_non_null(live.child);
	live.weak_child = obs_source_get_weak_source(live.child);
	return live.child;
}

static int64_t saved_int(obs_source_t *tr, const char *key)
{
	obs_data_release(live.saved);
	live.saved = obs_save_source(tr);
	return obs_data_get_int(live.saved, key);
}

/* Every field obs_transition_load() writes, from saved data holding each
 * key, and slot A and the actual size from the child it names. */
static void load_writes_each_field(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_data_t *tr_data = transition_data("fields");
	obs_data_set_string(tr_data, "transition_source_a", "fields A");
	obs_data_set_int(tr_data, "transition_alignment", OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM);
	obs_data_set_int(tr_data, "transition_mode", OBS_TRANSITION_MODE_MANUAL);
	obs_data_set_int(tr_data, "transition_scale_type", OBS_TRANSITION_SCALE_STRETCH);
	obs_data_set_int(tr_data, "transition_cx", 64);
	obs_data_set_int(tr_data, "transition_cy", 48);
	obs_data_t *items[] = {child_data("fields A"), tr_data};
	obs_source_t *tr = load(items, 2);
	obs_source_t *a = keep_child("fields A");

	assert_ptr_equal(slot(tr, OBS_TRANSITION_SOURCE_A), a);
	assert_null(slot(tr, OBS_TRANSITION_SOURCE_B));
	assert_int_equal(obs_transition_get_alignment(tr), OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM);
	assert_int_equal(obs_transition_get_scale_type(tr), OBS_TRANSITION_SCALE_STRETCH);
	uint32_t cx = 0, cy = 0;
	obs_transition_get_size(tr, &cx, &cy);
	assert_int_equal(cx, 64);
	assert_int_equal(cy, 48);
	assert_int_equal(saved_int(tr, "transition_mode"), OBS_TRANSITION_MODE_MANUAL);
	/* The actual size comes from the child, at once. */
	assert_int_equal(obs_source_get_width(tr), CHILD_CX);
	assert_int_equal(obs_source_get_height(tr), CHILD_CY);
	/* Loaded, not started. */
	assert_false(obs_transition_is_active(tr));
	assert_string_equal(warnings, "");
}

/* A key the data does not hold reads as 0, and 0 is written: the alignment
 * becomes OBS_ALIGN_CENTER rather than a new transition's left and top
 * (characterized, not endorsed). */
static void missing_keys_write_zero(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *probe = obs_source_create_private(TRANSITION_ID, "missing probe", NULL);
	assert_non_null(probe);
	assert_int_equal(obs_transition_get_alignment(probe), OBS_ALIGN_LEFT | OBS_ALIGN_TOP);
	obs_source_release(probe);

	obs_data_t *items[] = {transition_data("missing keys")};
	obs_source_t *tr = load(items, 1);

	assert_int_equal(obs_transition_get_alignment(tr), OBS_ALIGN_CENTER);
	assert_int_equal(obs_transition_get_scale_type(tr), OBS_TRANSITION_SCALE_MAX_ONLY);
	uint32_t cx = 1, cy = 1;
	obs_transition_get_size(tr, &cx, &cy);
	assert_int_equal(cx, 0);
	assert_int_equal(cy, 0);
	assert_int_equal(saved_int(tr, "transition_mode"), OBS_TRANSITION_MODE_AUTO);
	assert_null(slot(tr, OBS_TRANSITION_SOURCE_A));
	assert_int_equal(obs_source_get_width(tr), 0);
	assert_int_equal(obs_source_get_height(tr), 0);
	/* obs_data_get_string() gives "" for the missing name, which is
	 * looked up and not found (characterized, not endorsed). */
	assert_string_equal(warnings, "Failed to find source '' for transition 'missing keys'\n");
}

/* Values outside their types: the scale type is checked, the rest is cast
 * (characterized, not endorsed). */
static void out_of_range_values(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	static const int64_t scale_types[] = {3, -1, INT64_MAX, INT64_MIN};

	for (size_t i = 0; i < sizeof(scale_types) / sizeof(scale_types[0]); i++) {
		char name[32];
		snprintf(name, sizeof(name), "range %zu", i);
		obs_data_t *tr_data = transition_data(name);
		obs_data_set_int(tr_data, "transition_scale_type", scale_types[i]);
		obs_data_set_int(tr_data, "transition_alignment", 0x100000000LL | OBS_ALIGN_LEFT);
		obs_data_set_int(tr_data, "transition_mode", 7);
		obs_data_set_int(tr_data, "transition_cx", -1);
		obs_data_set_int(tr_data, "transition_cy", 0x100000005LL);
		obs_data_t *items[] = {tr_data};
		obs_source_t *tr = load(items, 1);

		assert_int_equal(obs_transition_get_scale_type(tr), OBS_TRANSITION_SCALE_MAX_ONLY);
		assert_int_equal(saved_int(tr, "transition_scale_type"), OBS_TRANSITION_SCALE_MAX_ONLY);
		assert_int_equal(obs_transition_get_alignment(tr), OBS_ALIGN_LEFT);
		assert_int_equal(saved_int(tr, "transition_mode"), 7);
		uint32_t cx = 0, cy = 0;
		obs_transition_get_size(tr, &cx, &cy);
		assert_int_equal(cx, UINT32_MAX);
		assert_int_equal(cy, 5);

		case_teardown(NULL);
	}
}

/* A found source goes into slot A as an active child: showing the
 * transition shows it, and the clear takes it back from the shown
 * transition. */
static void found_source_is_an_active_child(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_data_t *tr_data = transition_data("found");
	obs_data_set_string(tr_data, "transition_source_a", "found A");
	obs_data_t *items[] = {child_data("found A"), tr_data};
	obs_source_t *tr = load(items, 2);
	obs_source_t *a = keep_child("found A");
	assert_ptr_equal(slot(tr, OBS_TRANSITION_SOURCE_A), a);
	assert_false(obs_source_showing(a));

	obs_source_inc_showing(tr);
	live.shown = true;
	assert_true(obs_source_showing(a));

	obs_transition_clear(tr);
	assert_null(slot(tr, OBS_TRANSITION_SOURCE_A));
	assert_false(obs_source_showing(a));
	assert_string_equal(warnings, "");
}

/* A name no source has: a warning, and slot A stays empty. */
static void missing_source_is_logged(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_data_t *tr_data = transition_data("not found");
	obs_data_set_string(tr_data, "transition_source_a", "no such source");
	obs_data_t *items[] = {tr_data};
	obs_source_t *tr = load(items, 1);

	assert_null(slot(tr, OBS_TRANSITION_SOURCE_A));
	assert_string_equal(warnings, "Failed to find source 'no such source' for transition 'not found'\n");
}

/* The transition named as its own slot A: refused with a warning, slot A
 * empty, and the reference the lookup took given back (the transition goes
 * once the test lets go of it). */
static void the_transition_itself_is_refused(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_data_t *tr_data = transition_data("itself");
	obs_data_set_string(tr_data, "transition_source_a", "itself");
	obs_data_t *items[] = {tr_data};
	obs_source_t *tr = load(items, 1);

	assert_null(slot(tr, OBS_TRANSITION_SOURCE_A));
	/* The add's own warning, then the load's. */
	assert_string_equal(warnings, "obs_source_add_active_child: parent == child\n"
				      "Cannot set transition 'itself' to source 'itself' due to infinite recursion\n");

	obs_source_release(live.tr);
	live.tr = NULL;
	assert_true(gone(live.weak_tr));
}

/* The source slot A held before the load is released, whether the load
 * puts a source there or not. The callback for a leaf loaded before the
 * transition sets the transition to it, as a plugin can. */
static void check_old_source_released(const char *case_name, const char *source_a, const char *want_warning)
{
	char old_name[64], leaf_name[64], new_name[64];
	snprintf(old_name, sizeof(old_name), "%s old", case_name);
	snprintf(leaf_name, sizeof(leaf_name), "%s leaf", case_name);
	snprintf(new_name, sizeof(new_name), "%s new", case_name);

	live.old = obs_source_create_private(CHILD_ID, old_name, NULL);
	assert_non_null(live.old);
	live.weak_old = obs_source_get_weak_source(live.old);
	load_plan.set_from = leaf_name;
	load_plan.set_to = live.old;
	load_plan.tr_name = case_name;

	obs_data_t *tr_data = transition_data(case_name);
	if (source_a)
		obs_data_set_string(tr_data, "transition_source_a", source_a);
	obs_data_t *items[] = {child_data(leaf_name), child_data(new_name), tr_data};
	obs_source_t *tr = load(items, 3);
	memset(&load_plan, 0, sizeof(load_plan));

	obs_source_t *a = slot(tr, OBS_TRANSITION_SOURCE_A);
	if (source_a && strcmp(source_a, new_name) == 0) {
		assert_ptr_equal(a, keep_child(new_name));
	} else {
		assert_null(a);
	}
	assert_ptr_not_equal(a, live.old);

	/* Only the test holds the old source now. */
	obs_source_release(live.old);
	live.old = NULL;
	assert_true(gone(live.weak_old));
	assert_string_equal(warnings, want_warning);
	case_teardown(NULL);
	warnings[0] = '\0';
}

static void load_releases_the_old_source(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	check_old_source_released("replaced", "replaced new", "");
	check_old_source_released("emptied", "nothing", "Failed to find source 'nothing' for transition 'emptied'\n");
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	obs_register_source(&child_info);
	obs_register_source(&transition_info);
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(record_log, NULL);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up) {
		base_set_log_handler(prev_handler, prev_param);
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(load_writes_each_field, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(missing_keys_write_zero, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(out_of_range_values, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(found_source_is_an_active_child, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(missing_source_is_logged, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(the_transition_itself_is_refused, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(load_releases_the_old_source, case_setup, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

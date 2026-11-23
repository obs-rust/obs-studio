/* C-only characterization before extracting the source and canvas
 * enumerators (Refs #5493).
 *
 * test_enum_scenes already pins obs_canvas_enum_scenes through the
 * main-canvas wrapper obs_enum_scenes. This test pins the rest:
 *
 * - obs_enum_sources calls back for public inputs and public groups only
 *   (never private sources, scenes, filters or transitions), in creation
 *   order, each with one reference held for the call and released after; a
 *   source whose last reference is gone is skipped; a false return stops the
 *   walk with every reference released;
 * - obs_enum_all_sources calls back for every source, private ones included,
 *   in creation order, without taking references; a false return stops it;
 * - obs_enum_canvases calls back for the named (public) canvases, main
 *   first, in creation order, without taking references; private canvases
 *   are absent, a removed one stays until it is destroyed; a false return
 *   stops it;
 * - obs_canvas_enum_scenes on a canvas other than the main one calls back
 *   for that canvas's scenes only, in order, each referenced for the call;
 *   a false return stops it.
 *
 * Every case runs obs_startup() without obs_reset_video(), so no video
 * thread takes source references while they are counted, except the
 * transition case: a transition needs a graphics device to be created, so
 * that case resets video and checks only which sources are visited. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

#define INPUT_ID "test_enum_sources_input"
#define FILTER_ID "test_enum_sources_filter"
#define TRANSITION_ID "test_enum_sources_transition"
#define MAX_SEEN 16

/* Created in this order by populate(). */
enum {
	IN_A,
	PRIV_IN,
	SCENE,
	FILTER,
	GROUP,
	IN_B,
	PRIV_GROUP,
	PRIV_SCENE,
	N_SOURCES,
};

struct fixture {
	obs_source_t *sources[N_SOURCES];
	obs_source_t *transition;
	obs_canvas_t *canvases[3];
	obs_source_t *canvas_scenes[3];
	obs_source_t *main_scene;
};

struct visit {
	void *seen[MAX_SEEN];
	long refs[MAX_SEEN];
	size_t count, stop_after;
};

/* The main canvas outlives every case; the pointer is only compared. */
static obs_canvas_t *main_canvas(void)
{
	obs_canvas_t *canvas = obs_get_main_canvas();
	obs_canvas_release(canvas);
	return canvas;
}

static long source_refs(const obs_source_t *source)
{
	return os_atomic_load_long(&source->context.control->ref.refs);
}

static long canvas_refs(const obs_canvas_t *canvas)
{
	return os_atomic_load_long(&canvas->context.control->ref.refs);
}

static bool record_source(void *param, obs_source_t *source)
{
	struct visit *v = param;
	assert_true(v->count < MAX_SEEN);
	v->refs[v->count] = source_refs(source);
	v->seen[v->count++] = source;
	return !v->stop_after || v->count < v->stop_after;
}

static bool record_canvas(void *param, obs_canvas_t *canvas)
{
	struct visit *v = param;
	assert_true(v->count < MAX_SEEN);
	v->refs[v->count] = canvas_refs(canvas);
	v->seen[v->count++] = canvas;
	return !v->stop_after || v->count < v->stop_after;
}

static const char *type_name(void *data)
{
	(void)data;
	return "enum sources type";
}

static void *type_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void type_destroy(void *data)
{
	(void)data;
}

static uint32_t type_size(void *data)
{
	(void)data;
	return 16;
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
		.get_name = type_name,
		.create = type_create,
		.destroy = type_destroy,
		.get_width = type_size,
		.get_height = type_size,
	};
	obs_register_source(&input);
	struct obs_source_info filter = {
		.id = FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = type_name,
		.create = type_create,
		.destroy = type_destroy,
	};
	obs_register_source(&filter);
	struct obs_source_info transition = {
		.id = TRANSITION_ID,
		.type = OBS_SOURCE_TYPE_TRANSITION,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = type_name,
		.create = type_create,
		.destroy = type_destroy,
		.video_render = transition_render,
		.audio_render = transition_audio_render,
	};
	obs_register_source(&transition);
}

static int setup(void **state)
{
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	register_types();
	*state = bzalloc(sizeof(struct fixture));
	return 0;
}

static void remove_scene(obs_source_t *source)
{
	obs_scene_t *scene = obs_scene_from_source(source);
	obs_canvas_scene_remove(scene ? scene : obs_group_from_source(source));
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	if (f->sources[SCENE])
		remove_scene(f->sources[SCENE]);
	if (f->sources[GROUP])
		remove_scene(f->sources[GROUP]);
	if (f->main_scene)
		remove_scene(f->main_scene);
	for (size_t i = 0; i < 3; i++) {
		if (f->canvas_scenes[i])
			remove_scene(f->canvas_scenes[i]);
		obs_source_release(f->canvas_scenes[i]);
	}
	for (size_t i = 0; i < N_SOURCES; i++)
		obs_source_release(f->sources[i]);
	obs_source_release(f->main_scene);
	obs_source_release(f->transition);
	obs_wait_for_destroy_queue();
	for (size_t i = 0; i < 3; i++) {
		if (f->canvases[i] && !obs_canvas_removed(f->canvases[i]))
			obs_canvas_remove(f->canvases[i]);
		obs_canvas_release(f->canvases[i]);
	}
	obs_wait_for_destroy_queue();
	obs_shutdown();
	bfree(f);
	return 0;
}

static void populate(struct fixture *f)
{
	f->sources[IN_A] = obs_source_create(INPUT_ID, "enum input a", NULL, NULL);
	f->sources[PRIV_IN] = obs_source_create_private(INPUT_ID, "enum private input", NULL);
	f->sources[SCENE] = obs_scene_get_source(obs_scene_create("enum scene"));
	f->sources[FILTER] = obs_source_create(FILTER_ID, "enum filter", NULL, NULL);
	f->sources[GROUP] = obs_source_create("group", "enum group", NULL, NULL);
	f->sources[IN_B] = obs_source_create(INPUT_ID, "enum input b", NULL, NULL);
	f->sources[PRIV_GROUP] = obs_source_create_private("group", "enum private group", NULL);
	f->sources[PRIV_SCENE] = obs_scene_get_source(obs_scene_create_private("enum private scene"));
	for (size_t i = 0; i < N_SOURCES; i++)
		assert_non_null(f->sources[i]);
	assert_int_equal(obs_source_get_type(f->sources[FILTER]), OBS_SOURCE_TYPE_FILTER);
	assert_int_equal(obs_source_get_type(f->sources[GROUP]), OBS_SOURCE_TYPE_SCENE);
}

static void snapshot(struct fixture *f, long before[N_SOURCES])
{
	for (size_t i = 0; i < N_SOURCES; i++)
		before[i] = source_refs(f->sources[i]);
}

static size_t index_of(struct fixture *f, void *source)
{
	for (size_t i = 0; i < N_SOURCES; i++)
		if (f->sources[i] == source)
			return i;
	fail_msg("visited a source the test did not create");
	return N_SOURCES;
}

static void assert_refs_unchanged(struct fixture *f, const long before[N_SOURCES])
{
	for (size_t i = 0; i < N_SOURCES; i++)
		assert_int_equal(source_refs(f->sources[i]), before[i]);
}

static void empty_core(void **state)
{
	(void)state;
	struct visit v = {0};
	obs_enum_sources(record_source, &v);
	assert_int_equal(v.count, 0);
	obs_enum_all_sources(record_source, &v);
	assert_int_equal(v.count, 0);
	obs_enum_canvases(record_canvas, &v);
	assert_int_equal(v.count, 1);
	assert_ptr_equal(v.seen[0], main_canvas());
}

static void enum_sources_visits_public_inputs_and_groups(void **state)
{
	struct fixture *f = *state;
	populate(f);
	long before[N_SOURCES];
	snapshot(f, before);

	const size_t expected[] = {IN_A, GROUP, IN_B};
	struct visit v = {0};
	obs_enum_sources(record_source, &v);
	assert_int_equal(v.count, 3);
	for (size_t i = 0; i < 3; i++) {
		assert_ptr_equal(v.seen[i], f->sources[expected[i]]);
		/* One reference held for the call. */
		assert_int_equal(v.refs[i], before[expected[i]] + 1);
	}
	assert_refs_unchanged(f, before);
}

static void enum_sources_stops_on_false(void **state)
{
	struct fixture *f = *state;
	populate(f);
	long before[N_SOURCES];
	snapshot(f, before);

	const size_t expected[] = {IN_A, GROUP, IN_B};
	/* Stopping on the input, on the group and on the last one. */
	for (size_t stop = 1; stop <= 3; stop++) {
		struct visit v = {.stop_after = stop};
		obs_enum_sources(record_source, &v);
		assert_int_equal(v.count, stop);
		for (size_t i = 0; i < stop; i++) {
			assert_ptr_equal(v.seen[i], f->sources[expected[i]]);
			assert_int_equal(v.refs[i], before[expected[i]] + 1);
		}
		assert_refs_unchanged(f, before);
	}
}

static void enum_sources_skips_source_without_references(void **state)
{
	struct fixture *f = *state;
	populate(f);
	/* The window between a source's last release and its removal from the
	 * table: no reference can be taken, so the walk passes over it. */
	long saved = source_refs(f->sources[IN_A]);
	os_atomic_set_long(&f->sources[IN_A]->context.control->ref.refs, -1);

	struct visit v = {0};
	obs_enum_sources(record_source, &v);
	struct visit all = {0};
	obs_enum_all_sources(record_source, &all);
	os_atomic_set_long(&f->sources[IN_A]->context.control->ref.refs, saved);

	assert_int_equal(v.count, 2);
	assert_ptr_equal(v.seen[0], f->sources[GROUP]);
	assert_ptr_equal(v.seen[1], f->sources[IN_B]);
	assert_int_equal(all.count, N_SOURCES);
	assert_ptr_equal(all.seen[0], f->sources[IN_A]);
	assert_int_equal(all.refs[0], -1);
}

static void enum_all_sources_visits_every_source_without_refs(void **state)
{
	struct fixture *f = *state;
	populate(f);
	long before[N_SOURCES];
	snapshot(f, before);

	struct visit v = {0};
	obs_enum_all_sources(record_source, &v);
	assert_int_equal(v.count, N_SOURCES);
	for (size_t i = 0; i < N_SOURCES; i++) {
		assert_ptr_equal(v.seen[i], f->sources[i]);
		assert_int_equal(v.refs[i], before[i]);
	}

	for (size_t stop = 1; stop <= N_SOURCES; stop++) {
		struct visit s = {.stop_after = stop};
		obs_enum_all_sources(record_source, &s);
		assert_int_equal(s.count, stop);
		for (size_t i = 0; i < stop; i++)
			assert_int_equal(index_of(f, s.seen[i]), i);
	}
	assert_refs_unchanged(f, before);
}

static void enum_sources_skips_transitions(void **state)
{
	struct fixture *f = *state;
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
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_SUCCESS);
	f->sources[IN_A] = obs_source_create(INPUT_ID, "enum input a", NULL, NULL);
	f->transition = obs_source_create(TRANSITION_ID, "enum transition", NULL, NULL);
	f->sources[IN_B] = obs_source_create(INPUT_ID, "enum input b", NULL, NULL);
	assert_non_null(f->sources[IN_A]);
	assert_non_null(f->transition);
	assert_non_null(f->sources[IN_B]);
	assert_int_equal(obs_source_get_type(f->transition), OBS_SOURCE_TYPE_TRANSITION);

	/* The video thread references sources while it ticks them, so only the
	 * visited sources are checked here. */
	struct visit v = {0};
	obs_enum_sources(record_source, &v);
	assert_int_equal(v.count, 2);
	assert_ptr_equal(v.seen[0], f->sources[IN_A]);
	assert_ptr_equal(v.seen[1], f->sources[IN_B]);

	struct visit all = {0};
	obs_enum_all_sources(record_source, &all);
	assert_int_equal(all.count, 3);
	assert_ptr_equal(all.seen[0], f->sources[IN_A]);
	assert_ptr_equal(all.seen[1], f->transition);
	assert_ptr_equal(all.seen[2], f->sources[IN_B]);
}

static void enum_canvases_visits_named_canvases(void **state)
{
	struct fixture *f = *state;
	obs_canvas_t *main = main_canvas();
	f->canvases[0] = obs_canvas_create("enum canvas x", NULL, 0);
	f->canvases[1] = obs_canvas_create_private("enum private canvas", NULL, 0);
	f->canvases[2] = obs_canvas_create("enum canvas y", NULL, 0);
	for (size_t i = 0; i < 3; i++)
		assert_non_null(f->canvases[i]);
	obs_canvas_t *expected[] = {main, f->canvases[0], f->canvases[2]};
	long before[3];
	for (size_t i = 0; i < 3; i++)
		before[i] = canvas_refs(expected[i]);

	struct visit v = {0};
	obs_enum_canvases(record_canvas, &v);
	assert_int_equal(v.count, 3);
	for (size_t i = 0; i < 3; i++) {
		assert_ptr_equal(v.seen[i], expected[i]);
		/* No reference taken for the call. */
		assert_int_equal(v.refs[i], before[i]);
	}

	for (size_t stop = 1; stop <= 3; stop++) {
		struct visit s = {.stop_after = stop};
		obs_enum_canvases(record_canvas, &s);
		assert_int_equal(s.count, stop);
		for (size_t i = 0; i < stop; i++)
			assert_ptr_equal(s.seen[i], expected[i]);
	}
	for (size_t i = 0; i < 3; i++)
		assert_int_equal(canvas_refs(expected[i]), before[i]);

	/* A removed canvas stays in the walk while it is referenced, and
	 * leaves it once destroyed. */
	obs_canvas_remove(f->canvases[0]);
	assert_true(obs_canvas_removed(f->canvases[0]));
	struct visit r = {0};
	obs_enum_canvases(record_canvas, &r);
	assert_int_equal(r.count, 3);
	for (size_t i = 0; i < 3; i++)
		assert_ptr_equal(r.seen[i], expected[i]);
	obs_canvas_release(f->canvases[0]);
	f->canvases[0] = NULL;
	obs_wait_for_destroy_queue();
	struct visit d = {0};
	obs_enum_canvases(record_canvas, &d);
	assert_int_equal(d.count, 2);
	assert_ptr_equal(d.seen[0], main);
	assert_ptr_equal(d.seen[1], f->canvases[2]);
}

static void canvas_enum_scenes_on_other_canvas(void **state)
{
	struct fixture *f = *state;
	f->canvases[0] = obs_canvas_create("enum other canvas", NULL, 0);
	assert_non_null(f->canvases[0]);
	obs_canvas_t *other = f->canvases[0];
	f->canvas_scenes[0] = obs_scene_get_source(obs_canvas_scene_create(other, "enum other scene a"));
	f->main_scene = obs_scene_get_source(obs_scene_create("enum main scene"));
	f->canvas_scenes[1] = obs_scene_get_source(obs_canvas_scene_create(other, "enum other scene b"));
	f->canvas_scenes[2] = obs_scene_get_source(obs_canvas_scene_create(other, "enum other scene c"));
	for (size_t i = 0; i < 3; i++)
		assert_non_null(f->canvas_scenes[i]);
	assert_non_null(f->main_scene);
	long before[3];
	for (size_t i = 0; i < 3; i++)
		before[i] = source_refs(f->canvas_scenes[i]);
	long main_before = source_refs(f->main_scene);

	struct visit v = {0};
	obs_canvas_enum_scenes(other, record_source, &v);
	assert_int_equal(v.count, 3);
	for (size_t i = 0; i < 3; i++) {
		assert_ptr_equal(v.seen[i], f->canvas_scenes[i]);
		assert_int_equal(v.refs[i], before[i] + 1);
	}

	for (size_t stop = 1; stop <= 3; stop++) {
		struct visit s = {.stop_after = stop};
		obs_canvas_enum_scenes(other, record_source, &s);
		assert_int_equal(s.count, stop);
		for (size_t i = 0; i < stop; i++) {
			assert_ptr_equal(s.seen[i], f->canvas_scenes[i]);
			assert_int_equal(s.refs[i], before[i] + 1);
		}
	}
	for (size_t i = 0; i < 3; i++)
		assert_int_equal(source_refs(f->canvas_scenes[i]), before[i]);
	assert_int_equal(source_refs(f->main_scene), main_before);

	/* The main canvas walk does not see the other canvas's scenes. */
	struct visit m = {0};
	obs_canvas_enum_scenes(main_canvas(), record_source, &m);
	assert_int_equal(m.count, 1);
	assert_ptr_equal(m.seen[0], f->main_scene);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(empty_core, setup, teardown),
		cmocka_unit_test_setup_teardown(enum_sources_visits_public_inputs_and_groups, setup, teardown),
		cmocka_unit_test_setup_teardown(enum_sources_stops_on_false, setup, teardown),
		cmocka_unit_test_setup_teardown(enum_sources_skips_source_without_references, setup, teardown),
		cmocka_unit_test_setup_teardown(enum_all_sources_visits_every_source_without_refs, setup, teardown),
		cmocka_unit_test_setup_teardown(enum_sources_skips_transitions, setup, teardown),
		cmocka_unit_test_setup_teardown(enum_canvases_visits_named_canvases, setup, teardown),
		cmocka_unit_test_setup_teardown(canvas_enum_scenes_on_other_canvas, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

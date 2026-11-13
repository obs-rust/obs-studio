/* Characterize the public main-canvas wrapper before its Rust swap (#4801).
 * No audio/video reset: all ownership checks are synchronous. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

struct fixture {
	obs_canvas_t *main, *other;
	obs_source_t *sources[7];
};

struct visit {
	struct fixture *fixture;
	obs_source_t *seen[4], *held[4];
	long before[4];
	size_t count, stop_after;
	bool acquire;
};

static long refs(const obs_source_t *source)
{
	return os_atomic_load_long(&source->context.control->ref.refs);
}

static bool record_scene(void *param, obs_source_t *source)
{
	struct visit *v = param;
	assert_true(v->count < 4);
	size_t i = 0;
	while (i < 4 && v->fixture->sources[i] != source)
		i++;
	assert_true(i < 4);
	assert_int_equal(refs(source), v->before[i] + 1);
	v->seen[v->count] = source;
	if (v->acquire)
		v->held[v->count] = obs_source_get_ref(source);
	v->count++;
	return !v->stop_after || v->count < v->stop_after;
}

static const char *input_name(void *data)
{
	(void)data;
	return "enum scenes input";
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

static uint32_t input_size(void *data)
{
	(void)data;
	return 16;
}

static int setup(void **state)
{
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct fixture *f = bzalloc(sizeof(*f));
	*state = f;
	f->main = obs_get_main_canvas();
	f->other = obs_canvas_create_private("enum other canvas", NULL, 0);
	assert_non_null(f->main);
	assert_non_null(f->other);
	return 0;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	for (size_t i = 0; i < 7; i++) {
		if (f->sources[i] && (i < 4 || i == 5))
			obs_canvas_scene_remove(i == 3 ? obs_group_from_source(f->sources[i])
						       : obs_scene_from_source(f->sources[i]));
		obs_source_release(f->sources[i]);
	}
	obs_wait_for_destroy_queue();
	obs_canvas_release(f->other);
	obs_canvas_release(f->main);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	bfree(f);
	return 0;
}

static void populate(struct fixture *f)
{
	struct obs_source_info input = {
		.id = "enum_scenes_input",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_REQUIRES_CANVAS,
		.get_name = input_name,
		.create = input_create,
		.destroy = input_destroy,
		.get_width = input_size,
		.get_height = input_size,
	};
	obs_register_source(&input);
	const char *names[] = {"enum scene a", "enum scene b", "enum scene c"};
	for (size_t i = 0; i < 3; i++)
		f->sources[i] = obs_scene_get_source(obs_scene_create(names[i]));
	/* Characterized, not endorsed: groups share SCENE type and are included.
	 * Private scenes are absent from the canvas list. */
	f->sources[4] = obs_scene_get_source(obs_scene_create_private("enum private scene"));
	f->sources[5] = obs_scene_get_source(obs_canvas_scene_create(f->other, "enum other scene"));
	f->sources[6] = obs_source_create(input.id, "enum input", NULL, NULL);
	f->sources[3] = obs_source_create("group", "enum group", NULL, NULL);
	for (size_t i = 0; i < 7; i++)
		assert_non_null(f->sources[i]);
}

static struct visit visits(struct fixture *f, size_t stop_after, bool acquire)
{
	struct visit v = {.fixture = f, .stop_after = stop_after, .acquire = acquire};
	for (size_t i = 0; i < 4; i++)
		v.before[i] = f->sources[i] ? refs(f->sources[i]) : 0;
	return v;
}

static void empty_canvas(void **state)
{
	struct fixture *f = *state;
	struct visit v = visits(f, 0, false);
	obs_enum_scenes(record_scene, &v);
	assert_int_equal(v.count, 0);
	obs_canvas_enum_scenes(f->main, record_scene, &v);
	assert_int_equal(v.count, 0);
}

static void filtering_order_and_ownership(void **state)
{
	struct fixture *f = *state;
	populate(f);
	long before[7], active[7], showing[7];
	for (size_t i = 0; i < 7; i++) {
		before[i] = refs(f->sources[i]);
		active[i] = os_atomic_load_long(&f->sources[i]->activate_refs);
		showing[i] = os_atomic_load_long(&f->sources[i]->show_refs);
	}
	long canvas_refs = os_atomic_load_long(&f->main->context.control->ref.refs);
	struct visit direct = visits(f, 0, false);
	obs_canvas_enum_scenes(f->main, record_scene, &direct);
	assert_int_equal(direct.count, 4);
	for (size_t i = 0; i < 4; i++)
		assert_ptr_equal(direct.seen[i], f->sources[i]);
	for (size_t stop = 0; stop <= 4; stop++) {
		struct visit v = visits(f, stop, true);
		obs_enum_scenes(record_scene, &v);
		assert_int_equal(v.count, stop ? stop : 4);
		for (size_t i = 0; i < v.count; i++) {
			assert_ptr_equal(v.seen[i], direct.seen[i]);
			assert_ptr_equal(v.held[i], direct.seen[i]);
			assert_int_equal(refs(v.held[i]), before[i] + 1);
			obs_source_release(v.held[i]);
		}
		for (size_t i = 0; i < 7; i++) {
			assert_int_equal(refs(f->sources[i]), before[i]);
			assert_int_equal(os_atomic_load_long(&f->sources[i]->activate_refs), active[i]);
			assert_int_equal(os_atomic_load_long(&f->sources[i]->show_refs), showing[i]);
		}
		assert_int_equal(os_atomic_load_long(&f->main->context.control->ref.refs), canvas_refs);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(empty_canvas, setup, teardown),
		cmocka_unit_test_setup_teardown(filtering_order_and_ownership, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

/* Characterize owned canvas acquisition before the Rust swap (Refs #5123). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

#if !__has_include("obs-source-canvas-getter.c.h")
#error "source canvas getter extraction seam is missing"
#endif

static void expect_owned_canvas(obs_source_t *source, obs_canvas_t *expected)
{
	obs_weak_canvas_t *weak = source->canvas;
	assert_non_null(weak);
	long refs = os_atomic_load_long(&weak->ref.refs);
	long weak_refs = os_atomic_load_long(&weak->ref.weak_refs);
	obs_canvas_t *first = obs_source_get_canvas(source);
	obs_canvas_t *second = obs_source_get_canvas(source);
	assert_ptr_equal(first, expected);
	assert_ptr_equal(second, expected);
	assert_int_equal(os_atomic_load_long(&weak->ref.refs), refs + 2);
	assert_int_equal(os_atomic_load_long(&weak->ref.weak_refs), weak_refs);
	assert_ptr_equal(source->canvas, weak);
	obs_canvas_release(first);
	assert_int_equal(os_atomic_load_long(&weak->ref.refs), refs + 1);
	obs_canvas_release(second);
	assert_int_equal(os_atomic_load_long(&weak->ref.refs), refs);
}

static void main_canvas(void **unused)
{
	(void)unused;
	obs_canvas_t *canvas = obs_get_main_canvas();
	assert_non_null(canvas);
	obs_scene_t *scene = obs_scene_create("canvas getter main");
	assert_non_null(scene);
	expect_owned_canvas(obs_scene_get_source(scene), canvas);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_wait_for_destroy_queue();
	obs_canvas_release(canvas);
}

static void private_canvas(void **unused)
{
	(void)unused;
	obs_canvas_t *canvas = obs_canvas_create_private("canvas getter private", NULL, PROGRAM);
	assert_non_null(canvas);
	obs_scene_t *scene = obs_canvas_scene_create(canvas, "canvas getter private scene");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	expect_owned_canvas(source, canvas);
	obs_canvas_t *owned = obs_source_get_canvas(source);
	assert_ptr_equal(owned, canvas);
	obs_canvas_release(canvas);
	/* The returned strong reference alone keeps the weak target alive. */
	expect_owned_canvas(source, owned);
	obs_source_remove(source);
	obs_scene_release(scene);
	obs_wait_for_destroy_queue();
	obs_canvas_release(owned);
}

static void moved_source(void **unused)
{
	(void)unused;
	obs_canvas_t *from = obs_canvas_create_private("canvas getter from", NULL, 0);
	obs_canvas_t *to = obs_canvas_create_private("canvas getter to", NULL, PROGRAM);
	assert_non_null(from);
	assert_non_null(to);
	obs_scene_t *scene = obs_canvas_scene_create(from, "canvas getter moved");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	expect_owned_canvas(source, from);
	obs_canvas_t *old_owner = obs_source_get_canvas(source);
	assert_ptr_equal(old_owner, from);
	obs_canvas_move_scene(scene, to);
	expect_owned_canvas(source, to);
	/* A previously returned reference still owns the original canvas. */
	obs_canvas_release(from);
	obs_canvas_t *old_ref = obs_canvas_get_ref(old_owner);
	assert_ptr_equal(old_ref, old_owner);
	obs_canvas_release(old_ref);
	obs_canvas_release(old_owner);
	obs_canvas_move_scene(scene, to);
	expect_owned_canvas(source, to);
	obs_source_remove(source);
	obs_scene_release(scene);
	obs_wait_for_destroy_queue();
	obs_canvas_release(to);
}

static void null_weak_canvas(void **unused)
{
	(void)unused;
	/* The getter reads only canvas; this synthetic source owns no state.
	 * A NULL source itself is outside the original C contract. */
	struct obs_source source = {0};
	assert_null(obs_source_get_canvas(&source));
	assert_null(source.canvas);
}

static void expired_weak_canvas(void **unused)
{
	(void)unused;
	obs_canvas_t *canvas = obs_canvas_create_private("canvas getter expired", NULL, 0);
	assert_non_null(canvas);
	/* Retain the real weak control after its canvas is destroyed. */
	struct obs_source source = {0};
	source.canvas = obs_canvas_get_weak_canvas(canvas);
	assert_non_null(source.canvas);
	obs_canvas_release(canvas);
	long weak_refs = os_atomic_load_long(&source.canvas->ref.weak_refs);
	assert_int_equal(os_atomic_load_long(&source.canvas->ref.refs), -1);
	assert_null(obs_source_get_canvas(&source));
	assert_null(obs_source_get_canvas(&source));
	assert_int_equal(os_atomic_load_long(&source.canvas->ref.refs), -1);
	assert_int_equal(os_atomic_load_long(&source.canvas->ref.weak_refs), weak_refs);
	obs_weak_canvas_release(source.canvas);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(main_canvas),         cmocka_unit_test(private_canvas),
		cmocka_unit_test(moved_source),        cmocka_unit_test(null_weak_canvas),
		cmocka_unit_test(expired_weak_canvas),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return result;
}

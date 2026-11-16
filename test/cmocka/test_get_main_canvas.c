/* C-only characterization before extracting obs_get_main_canvas (Refs #4779).
 * An initialized core is a precondition; no audio/video reset is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <cmocka.h>
#include "obs-internal.h"

static void identity_and_acquired_reference(void **state)
{
	(void)state;
	obs_canvas_t *expected = obs_get_canvas_by_name("Main");
	assert_non_null(expected);
	long refs = os_atomic_load_long(&expected->context.control->ref.refs);
	long weak_refs = os_atomic_load_long(&expected->context.control->ref.weak_refs);

	obs_canvas_t *first = obs_get_main_canvas();
	assert_ptr_equal(first, expected);
	assert_int_equal(os_atomic_load_long(&expected->context.control->ref.refs), refs + 1);
	obs_canvas_t *second = obs_get_main_canvas();
	assert_ptr_equal(second, expected);
	assert_int_equal(os_atomic_load_long(&expected->context.control->ref.refs), refs + 2);
	assert_int_equal(os_atomic_load_long(&expected->context.control->ref.weak_refs), weak_refs);
	assert_string_equal(obs_canvas_get_name(first), "Main");
	assert_false(obs_canvas_removed(first));
	assert_null(obs_canvas_get_video(first));

	obs_canvas_release(first);
	assert_int_equal(os_atomic_load_long(&expected->context.control->ref.refs), refs + 1);
	obs_canvas_release(second);
	assert_int_equal(os_atomic_load_long(&expected->context.control->ref.refs), refs);
	obs_canvas_release(expected);
}

static void repeated_reads_preserve_core_ownership(void **state)
{
	(void)state;
	obs_canvas_t *canvas = obs_get_main_canvas();
	assert_non_null(canvas);
	obs_weak_canvas_t *weak = obs_canvas_get_weak_canvas(canvas);
	assert_non_null(weak);
	struct obs_weak_object *control = canvas->context.control;
	long refs = os_atomic_load_long(&control->ref.refs);
	long weak_refs = os_atomic_load_long(&control->ref.weak_refs);
	obs_canvas_t *held[8];
	for (size_t i = 0; i < OBS_COUNTOF(held); ++i) {
		held[i] = obs_get_main_canvas();
		assert_ptr_equal(held[i], canvas);
		assert_int_equal(os_atomic_load_long(&control->ref.refs), refs + (long)i + 1);
		assert_int_equal(os_atomic_load_long(&control->ref.weak_refs), weak_refs);
	}
	for (size_t i = OBS_COUNTOF(held); i > 0; --i) {
		obs_canvas_release(held[i - 1]);
		assert_int_equal(os_atomic_load_long(&control->ref.refs), refs + (long)i - 1);
	}
	obs_canvas_release(canvas);
	assert_int_equal(os_atomic_load_long(&control->ref.refs), refs - 1);
	/* The core still owns the canvas after every caller releases its read. */
	canvas = obs_weak_canvas_get_canvas(weak);
	assert_non_null(canvas);
	assert_int_equal(os_atomic_load_long(&control->ref.refs), refs);
	obs_canvas_t *again = obs_get_main_canvas();
	assert_ptr_equal(again, canvas);
	obs_canvas_release(again);
	obs_canvas_release(canvas);
	obs_weak_canvas_release(weak);
}

static void auxiliary_canvases_do_not_change_main(void **state)
{
	(void)state;
	obs_canvas_t *main = obs_get_main_canvas();
	assert_non_null(main);
	long refs = os_atomic_load_long(&main->context.control->ref.refs);
	obs_canvas_t *public = obs_canvas_create("getter public", NULL, 0);
	obs_canvas_t *private = obs_canvas_create_private("getter private", NULL, 0);
	assert_non_null(public);
	assert_non_null(private);
	assert_ptr_not_equal(main, public);
	assert_ptr_not_equal(main, private);
	long public_refs = os_atomic_load_long(&public->context.control->ref.refs);
	long private_refs = os_atomic_load_long(&private->context.control->ref.refs);
	obs_canvas_t *read = obs_get_main_canvas();
	assert_ptr_equal(read, main);
	assert_int_equal(os_atomic_load_long(&main->context.control->ref.refs), refs + 1);
	assert_int_equal(os_atomic_load_long(&public->context.control->ref.refs), public_refs);
	assert_int_equal(os_atomic_load_long(&private->context.control->ref.refs), private_refs);
	obs_canvas_release(read);
	obs_canvas_release(public);
	obs_canvas_release(private);
	read = obs_get_main_canvas();
	assert_ptr_equal(read, main);
	obs_canvas_release(read);
	assert_int_equal(os_atomic_load_long(&main->context.control->ref.refs), refs);
	obs_canvas_release(main);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 77;
	}
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(identity_and_acquired_reference),
		cmocka_unit_test(repeated_reads_preserve_core_ownership),
		cmocka_unit_test(auxiliary_canvases_do_not_change_main),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}

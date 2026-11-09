/*
 * Characterize the display's create, destroy, resize, color-space flag and
 * draw callback calls (libobs/obs-display.c) before the display seam
 * (obs-rust/obs-studio#4140) copies the field, lock, container and list
 * lines they run. On a started core with no video (no graphics context) and
 * displays made with obs_display_create(NULL, color) (no swap chain), it
 * pins what each public call leaves in struct obs_display:
 * - create: the fields it sets and the ones it leaves zero, both mutexes
 *   made and let go, and the new display linked at the front of the core's
 *   display list (next, and prev_next pointing at the link that holds it);
 * - destroy: the display unlinked from the front, middle and end of the
 *   list, and everything create and the callbacks allocated freed;
 * - resize: next_cx and next_cy set, cx and cy left alone, draw_info_mutex
 *   let go;
 * - update_color_space: the flag set, draw_info_mutex let go;
 * - add and remove of draw callbacks: pushed in order, a pair added twice
 *   kept twice, remove taking the first match only, draw_callbacks_mutex let
 *   go.
 * The NULL display calls doing nothing are characterized, not endorsed.
 * use_clear_workaround is left out: it comes from the host
 * (obs_host_display_clear_workaround()), so it differs by platform.
 *
 * obs_startup() needs a display on Linux; without one each case skips.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include "obs-internal.h"

static int tag;
static int other_tag;

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return 0;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

#define REQUIRE_OBS()                                                              \
	do {                                                                       \
		if (!obs_initialized()) {                                          \
			fprintf(stderr, "skipped: obs_startup failed (no display?)\n"); \
			skip();                                                    \
		}                                                                  \
	} while (0)

/* The mutex is made and not held: a trylock from this thread succeeds. */
static void check_free(pthread_mutex_t *mutex)
{
	assert_int_equal(pthread_mutex_trylock(mutex), 0);
	assert_int_equal(pthread_mutex_unlock(mutex), 0);
}

static void test_create(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_display_t *display = obs_display_create(NULL, 0xff336699);
	assert_non_null(display);

	assert_int_equal(display->background_color, 0xff336699);
	assert_true(display->enabled);
	assert_false(display->update_color_space);
	/* No graphics data: no swap chain and no size. */
	assert_null(display->swap);
	assert_int_equal(display->cx, 0);
	assert_int_equal(display->cy, 0);
	assert_int_equal(display->next_cx, 0);
	assert_int_equal(display->next_cy, 0);
	assert_int_equal(display->draw_callbacks.num, 0);
	assert_null(display->draw_callbacks.array);
	check_free(&display->draw_callbacks_mutex);
	check_free(&display->draw_info_mutex);

	/* Alone on the list: the head link holds it, and nothing follows. */
	assert_non_null(display->prev_next);
	assert_ptr_equal(*display->prev_next, display);
	assert_null(display->next);

	obs_display_destroy(display);
}

static void test_list(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_display_t *a = obs_display_create(NULL, 1);
	assert_non_null(a);
	obs_display_t **head = a->prev_next;
	obs_display_t *b = obs_display_create(NULL, 2);
	obs_display_t *c = obs_display_create(NULL, 3);
	assert_non_null(b);
	assert_non_null(c);

	/* Each new display goes to the front, at the same head link: c, b, a. */
	assert_ptr_equal(c->prev_next, head);
	assert_ptr_equal(*head, c);
	assert_ptr_equal(c->next, b);
	assert_ptr_equal(b->prev_next, &c->next);
	assert_ptr_equal(b->next, a);
	assert_ptr_equal(a->prev_next, &b->next);
	assert_null(a->next);

	/* The middle one out: c, a. */
	obs_display_destroy(b);
	assert_ptr_equal(*head, c);
	assert_ptr_equal(c->next, a);
	assert_ptr_equal(a->prev_next, &c->next);
	assert_null(a->next);

	/* The first one out: a. */
	obs_display_destroy(c);
	assert_ptr_equal(*head, a);
	assert_ptr_equal(a->prev_next, head);
	assert_null(a->next);

	/* The last one out: empty. */
	obs_display_destroy(a);
	assert_null(*head);

	obs_display_destroy(NULL);
}

static void test_resize(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_display_t *display = obs_display_create(NULL, 0);
	assert_non_null(display);

	obs_display_resize(display, 1280, 720);
	assert_int_equal(display->next_cx, 1280);
	assert_int_equal(display->next_cy, 720);
	/* The size render_display() last used is left for it to change. */
	assert_int_equal(display->cx, 0);
	assert_int_equal(display->cy, 0);
	check_free(&display->draw_info_mutex);

	obs_display_resize(display, 640, 480);
	assert_int_equal(display->next_cx, 640);
	assert_int_equal(display->next_cy, 480);

	obs_display_resize(NULL, 1, 1);

	obs_display_destroy(display);
}

static void test_update_color_space(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_display_t *display = obs_display_create(NULL, 0);
	assert_non_null(display);

	assert_false(display->update_color_space);
	obs_display_update_color_space(display);
	assert_true(display->update_color_space);
	check_free(&display->draw_info_mutex);
	/* Set again, it stays set until render_display() takes it. */
	obs_display_update_color_space(display);
	assert_true(display->update_color_space);

	obs_display_update_color_space(NULL);

	obs_display_destroy(display);
}

static void draw_a(void *param, uint32_t cx, uint32_t cy)
{
	(void)param;
	(void)cx;
	(void)cy;
}

static void draw_b(void *param, uint32_t cx, uint32_t cy)
{
	(void)param;
	(void)cx;
	(void)cy;
}

static void test_draw_callbacks(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_display_t *display = obs_display_create(NULL, 0);
	assert_non_null(display);

	/* A pair added twice is kept twice, in order. */
	obs_display_add_draw_callback(display, draw_a, &tag);
	obs_display_add_draw_callback(display, draw_b, &other_tag);
	obs_display_add_draw_callback(display, draw_a, &tag);
	assert_int_equal(display->draw_callbacks.num, 3);
	assert_ptr_equal(display->draw_callbacks.array[0].draw, draw_a);
	assert_ptr_equal(display->draw_callbacks.array[0].param, &tag);
	assert_ptr_equal(display->draw_callbacks.array[1].draw, draw_b);
	assert_ptr_equal(display->draw_callbacks.array[1].param, &other_tag);
	assert_ptr_equal(display->draw_callbacks.array[2].draw, draw_a);
	assert_ptr_equal(display->draw_callbacks.array[2].param, &tag);
	check_free(&display->draw_callbacks_mutex);

	/* Remove takes the first match only. */
	obs_display_remove_draw_callback(display, draw_a, &tag);
	assert_int_equal(display->draw_callbacks.num, 2);
	assert_ptr_equal(display->draw_callbacks.array[0].draw, draw_b);
	assert_ptr_equal(display->draw_callbacks.array[1].draw, draw_a);
	check_free(&display->draw_callbacks_mutex);

	/* A pair that matches on one field only stays. */
	obs_display_remove_draw_callback(display, draw_b, &tag);
	assert_int_equal(display->draw_callbacks.num, 2);

	obs_display_add_draw_callback(NULL, draw_a, &tag);
	obs_display_remove_draw_callback(NULL, draw_a, &tag);

	obs_display_destroy(display);
}

static void test_destroy_frees(void **state)
{
	(void)state;
	REQUIRE_OBS();

	long before = bnum_allocs();
	obs_display_t *display = obs_display_create(NULL, 0);
	assert_non_null(display);
	assert_true(bnum_allocs() > before);
	obs_display_add_draw_callback(display, draw_a, &tag);
	obs_display_add_draw_callback(display, draw_b, &other_tag);

	/* The display and its callback array. */
	obs_display_destroy(display);
	assert_int_equal(bnum_allocs(), before);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_create, setup, teardown),
		cmocka_unit_test_setup_teardown(test_list, setup, teardown),
		cmocka_unit_test_setup_teardown(test_resize, setup, teardown),
		cmocka_unit_test_setup_teardown(test_update_color_space, setup, teardown),
		cmocka_unit_test_setup_teardown(test_draw_callbacks, setup, teardown),
		cmocka_unit_test_setup_teardown(test_destroy_frees, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}

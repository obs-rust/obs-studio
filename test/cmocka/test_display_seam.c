/*
 * The display seam (libobs/obs-display-seam.h, obs-rust/obs-studio#4140),
 * which the port of the display (libobs/obs-display.c) calls in place of the
 * struct reads, writes, locks and container calls it wraps:
 * - the two mutexes: their init values, init and destroy, and the lock and
 *   unlock of draw_callbacks_mutex;
 * - the fields: use_clear_workaround, swap, next_cx, next_cy,
 *   update_color_space, enabled and background_color;
 * - the draw callbacks: push, erase item, count, item and free;
 * - the display list: displays_mutex, first_display and a display's next and
 *   prev_next links;
 * - the core video: the graphics context and the solid effect.
 *
 * Each read is checked against the field it wraps, each write is read back
 * from the field, each lock and unlock returns 0 and holds (or lets go of)
 * the mutex it names, the callback calls leave the array as da_push_back(),
 * da_erase_item() and da_free() do, and the list calls, run in the order
 * obs_display_create() and obs_display_destroy() run them, link and unlink
 * the displays as those do.
 *
 * The accessors are hidden in libobs, so this test compiles the seam into
 * itself and runs it on a stand-in core (`obs`, with only the fields the seam
 * reads) and stand-in displays: zeroed structs with their mutexes
 * initialized.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <string.h>

#include "obs-internal.h"
#include "obs-display-seam.h"

/* libobs has this one in obs.c, hidden; the seam in this test reads this
 * stand-in. */
struct obs_core *obs;

static int tag;
static int other_tag;

static obs_display_t *new_display(void)
{
	obs_display_t *display = bzalloc(sizeof(*display));
	assert_int_equal(pthread_mutex_init(&display->draw_callbacks_mutex, NULL), 0);
	assert_int_equal(pthread_mutex_init(&display->draw_info_mutex, NULL), 0);
	return display;
}

static void free_display(obs_display_t *display)
{
	da_free(display->draw_callbacks);
	pthread_mutex_destroy(&display->draw_callbacks_mutex);
	pthread_mutex_destroy(&display->draw_info_mutex);
	bfree(display);
}

static int setup(void **state)
{
	obs = bzalloc(sizeof(*obs));
	if (pthread_mutex_init(&obs->data.displays_mutex, NULL) != 0)
		return -1;
	(void)state;
	return 0;
}

static int teardown(void **state)
{
	pthread_mutex_destroy(&obs->data.displays_mutex);
	bfree(obs);
	obs = NULL;
	(void)state;
	return 0;
}

/* A lock holds the mutex (a trylock from this thread is refused) and its
 * unlock lets it go (a trylock then succeeds). */
static void check_held(pthread_mutex_t *mutex, int lock_result)
{
	assert_int_equal(lock_result, 0);
	assert_int_not_equal(pthread_mutex_trylock(mutex), 0);
}

static void check_released(pthread_mutex_t *mutex, int unlock_result)
{
	assert_int_equal(unlock_result, 0);
	assert_int_equal(pthread_mutex_trylock(mutex), 0);
	assert_int_equal(pthread_mutex_unlock(mutex), 0);
}

static void test_mutex_init_values(void **state)
{
	(void)state;
	obs_display_t *display = bzalloc(sizeof(*display));
	pthread_mutex_t init_val = PTHREAD_MUTEX_INITIALIZER;

	/* pthread_mutex_init_value() of both, over a mutex that isn't. */
	memset(&display->draw_callbacks_mutex, 0xa5, sizeof(display->draw_callbacks_mutex));
	memset(&display->draw_info_mutex, 0xa5, sizeof(display->draw_info_mutex));
	obs_display_seam_init_mutex_values(display);
	assert_memory_equal(&display->draw_callbacks_mutex, &init_val, sizeof(init_val));
	assert_memory_equal(&display->draw_info_mutex, &init_val, sizeof(init_val));

	bfree(display);
}

static void test_mutex_init_destroy(void **state)
{
	(void)state;
	obs_display_t *display = bzalloc(sizeof(*display));
	obs_display_seam_init_mutex_values(display);

	/* Each init makes a mutex that locks and unlocks. */
	assert_int_equal(obs_display_seam_callbacks_mutex_init(display), 0);
	assert_int_equal(obs_display_seam_info_mutex_init(display), 0);
	check_held(&display->draw_callbacks_mutex, pthread_mutex_lock(&display->draw_callbacks_mutex));
	check_released(&display->draw_callbacks_mutex, pthread_mutex_unlock(&display->draw_callbacks_mutex));
	check_held(&display->draw_info_mutex, pthread_mutex_lock(&display->draw_info_mutex));
	check_released(&display->draw_info_mutex, pthread_mutex_unlock(&display->draw_info_mutex));

	/* Destroy lets each go; the test destroys neither again. */
	obs_display_seam_callbacks_mutex_destroy(display);
	obs_display_seam_info_mutex_destroy(display);

	bfree(display);
}

static void test_callbacks_lock(void **state)
{
	(void)state;
	obs_display_t *display = new_display();

	check_held(&display->draw_callbacks_mutex, obs_display_seam_lock_callbacks(display));
	/* It is draw_callbacks_mutex, not draw_info_mutex. */
	assert_int_equal(pthread_mutex_trylock(&display->draw_info_mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&display->draw_info_mutex), 0);
	check_released(&display->draw_callbacks_mutex, obs_display_seam_unlock_callbacks(display));

	free_display(display);
}

static void test_fields(void **state)
{
	(void)state;
	obs_display_t *display = new_display();

	display->use_clear_workaround = true;
	assert_true(obs_display_seam_get_use_clear_workaround(display));
	obs_display_seam_set_use_clear_workaround(display, false);
	assert_false(display->use_clear_workaround);
	obs_display_seam_set_use_clear_workaround(display, true);
	assert_true(display->use_clear_workaround);

	display->swap = (gs_swapchain_t *)&tag;
	assert_ptr_equal(obs_display_seam_get_swap(display), &tag);
	obs_display_seam_set_swap(display, (gs_swapchain_t *)&other_tag);
	assert_ptr_equal(display->swap, &other_tag);
	obs_display_seam_set_swap(display, NULL);
	assert_null(display->swap);

	display->next_cx = 1920;
	display->next_cy = 1080;
	assert_int_equal(obs_display_seam_get_next_cx(display), 1920);
	assert_int_equal(obs_display_seam_get_next_cy(display), 1080);
	obs_display_seam_set_next_cx(display, 640);
	obs_display_seam_set_next_cy(display, 480);
	assert_int_equal(display->next_cx, 640);
	assert_int_equal(display->next_cy, 480);
	/* Neither touches the size render_display_begin() keeps. */
	assert_int_equal(display->cx, 0);
	assert_int_equal(display->cy, 0);

	display->update_color_space = true;
	assert_true(obs_display_seam_get_update_color_space(display));
	obs_display_seam_set_update_color_space(display, false);
	assert_false(display->update_color_space);
	obs_display_seam_set_update_color_space(display, true);
	assert_true(display->update_color_space);

	display->enabled = true;
	assert_true(obs_display_seam_get_enabled(display));
	obs_display_seam_set_enabled(display, false);
	assert_false(display->enabled);
	obs_display_seam_set_enabled(display, true);
	assert_true(display->enabled);

	display->background_color = 0xff336699;
	assert_int_equal(obs_display_seam_get_background_color(display), 0xff336699);
	obs_display_seam_set_background_color(display, 0x00112233);
	assert_int_equal(display->background_color, 0x00112233);

	free_display(display);
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

static void test_callbacks(void **state)
{
	(void)state;
	obs_display_t *display = new_display();
	struct draw_callback a = {draw_a, &tag};
	struct draw_callback b = {draw_b, &other_tag};

	/* A pair added twice is pushed twice. */
	obs_display_seam_callbacks_push(display, &a);
	obs_display_seam_callbacks_push(display, &b);
	obs_display_seam_callbacks_push(display, &a);
	assert_int_equal(display->draw_callbacks.num, 3);
	assert_ptr_equal(display->draw_callbacks.array[0].draw, draw_a);
	assert_ptr_equal(display->draw_callbacks.array[0].param, &tag);
	assert_ptr_equal(display->draw_callbacks.array[1].draw, draw_b);
	assert_ptr_equal(display->draw_callbacks.array[1].param, &other_tag);
	assert_ptr_equal(display->draw_callbacks.array[2].draw, draw_a);

	assert_int_equal(obs_display_seam_callbacks_num(display), 3);
	assert_ptr_equal(obs_display_seam_callbacks_get(display, 0), display->draw_callbacks.array);
	assert_ptr_equal(obs_display_seam_callbacks_get(display, 2), display->draw_callbacks.array + 2);

	/* da_erase_item() takes the first match only. */
	obs_display_seam_callbacks_erase_item(display, &a);
	assert_int_equal(display->draw_callbacks.num, 2);
	assert_ptr_equal(display->draw_callbacks.array[0].draw, draw_b);
	assert_ptr_equal(display->draw_callbacks.array[1].draw, draw_a);
	assert_int_equal(obs_display_seam_callbacks_num(display), 2);

	/* A pair that matches neither field leaves the array alone. */
	struct draw_callback mixed = {draw_b, &tag};
	obs_display_seam_callbacks_erase_item(display, &mixed);
	assert_int_equal(display->draw_callbacks.num, 2);

	obs_display_seam_callbacks_free(display);
	assert_int_equal(display->draw_callbacks.num, 0);
	assert_int_equal(display->draw_callbacks.capacity, 0);
	assert_null(display->draw_callbacks.array);
	assert_int_equal(obs_display_seam_callbacks_num(display), 0);

	free_display(display);
}

static void test_displays_lock(void **state)
{
	(void)state;
	check_held(&obs->data.displays_mutex, obs_display_seam_lock_displays());
	check_released(&obs->data.displays_mutex, obs_display_seam_unlock_displays());
}

/* obs_display_create()'s link, through the seam. */
static void link_display(obs_display_t *display)
{
	obs_display_seam_set_prev_next(display, obs_display_seam_first_display_ref());
	obs_display_seam_set_next(display, obs_display_seam_get_first_display());
	obs_display_seam_set_first_display(display);
	obs_display_t *next = obs_display_seam_get_next(display);
	if (next)
		obs_display_seam_set_prev_next(next, obs_display_seam_next_ref(display));
}

/* obs_display_destroy()'s unlink, through the seam. */
static void unlink_display(obs_display_t *display)
{
	obs_display_t **prev_next = obs_display_seam_get_prev_next(display);
	if (prev_next)
		*prev_next = obs_display_seam_get_next(display);
	obs_display_t *next = obs_display_seam_get_next(display);
	if (next)
		obs_display_seam_set_prev_next(next, obs_display_seam_get_prev_next(display));
}

static void test_display_list(void **state)
{
	(void)state;
	obs_display_t *a = new_display();
	obs_display_t *b = new_display();
	obs_display_t *c = new_display();

	assert_null(obs_display_seam_get_first_display());
	assert_ptr_equal(obs_display_seam_first_display_ref(), &obs->data.first_display);
	assert_ptr_equal(obs_display_seam_next_ref(a), &a->next);

	/* Each new display goes to the front: c, b, a. */
	link_display(a);
	link_display(b);
	link_display(c);
	assert_ptr_equal(obs->data.first_display, c);
	assert_ptr_equal(c->next, b);
	assert_ptr_equal(b->next, a);
	assert_null(a->next);
	assert_ptr_equal(c->prev_next, &obs->data.first_display);
	assert_ptr_equal(b->prev_next, &c->next);
	assert_ptr_equal(a->prev_next, &b->next);

	assert_ptr_equal(obs_display_seam_get_first_display(), c);
	assert_ptr_equal(obs_display_seam_get_next(c), b);
	assert_ptr_equal(obs_display_seam_get_prev_next(a), &b->next);

	/* The middle one out: c, a. */
	unlink_display(b);
	assert_ptr_equal(obs->data.first_display, c);
	assert_ptr_equal(c->next, a);
	assert_ptr_equal(a->prev_next, &c->next);

	/* The first one out: a. */
	unlink_display(c);
	assert_ptr_equal(obs->data.first_display, a);
	assert_ptr_equal(a->prev_next, &obs->data.first_display);

	/* The last one out: empty. */
	unlink_display(a);
	assert_null(obs->data.first_display);

	obs_display_seam_set_first_display(b);
	assert_ptr_equal(obs->data.first_display, b);
	obs_display_seam_set_first_display(NULL);
	assert_null(obs->data.first_display);

	free_display(a);
	free_display(b);
	free_display(c);
}

static void test_core_video(void **state)
{
	(void)state;
	obs->video.graphics = (graphics_t *)&tag;
	obs->video.solid_effect = (gs_effect_t *)&other_tag;
	assert_ptr_equal(obs_display_seam_get_graphics(), &tag);
	assert_ptr_equal(obs_display_seam_get_solid_effect(), &other_tag);
	obs->video.graphics = NULL;
	obs->video.solid_effect = NULL;
	assert_null(obs_display_seam_get_graphics());
	assert_null(obs_display_seam_get_solid_effect());
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_mutex_init_values, setup, teardown),
		cmocka_unit_test_setup_teardown(test_mutex_init_destroy, setup, teardown),
		cmocka_unit_test_setup_teardown(test_callbacks_lock, setup, teardown),
		cmocka_unit_test_setup_teardown(test_fields, setup, teardown),
		cmocka_unit_test_setup_teardown(test_callbacks, setup, teardown),
		cmocka_unit_test_setup_teardown(test_displays_lock, setup, teardown),
		cmocka_unit_test_setup_teardown(test_display_list, setup, teardown),
		cmocka_unit_test_setup_teardown(test_core_video, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}

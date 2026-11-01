/* obs_shutdown() must free the weak canvas reference of every source it
 * destroys (#2038).
 *
 * obs_free_data() releases the main canvas before it destroys the sources
 * the host left alive. obs_canvas_remove_source() released a source's weak
 * canvas reference only while the canvas was still alive, so each leftover
 * source leaked it, and with it the canvas's control block. The canvas also
 * never freed its source name table while sources were left in it (ASan:
 * obs-canvas.c.h obs_canvas_insert_source and obs_canvas_create_internal).
 *
 * Both are bmalloc allocations, so bnum_allocs() after obs_shutdown() shows
 * them. Public API only, so it runs against libobs, the hybrid libobs and
 * libobs-c-reference. obs_startup() needs a display on Linux; without one
 * the case skips. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/bmem.h>
#include <util/platform.h>

#define INPUT_ID "shutdown_canvas_weak_ref_input"

static const char *input_name(void *type_data)
{
	(void)type_data;
	return "Shutdown canvas weak ref input";
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

static void test_sources_left_alive_free_their_canvas_ref(void **state)
{
	(void)state;
	long before = bnum_allocs();
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		skip();
	}

	struct obs_source_info input = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = input_name,
		.create = input_create,
		.destroy = input_destroy,
	};
	obs_register_source(&input);

	/* Left alive on purpose: obs_free_data() destroys both. */
	assert_non_null(obs_source_create(INPUT_ID, "left alive", NULL, NULL));
	assert_non_null(obs_scene_create("left in the main canvas"));

	obs_shutdown();

	/* obs_free_data() gives back the reference the source held on its
	 * control block, as obs_source_release() does, so with no weak
	 * reference left the control block goes too (#3364). */
	assert_int_equal(bnum_allocs() - before, 0);
}

/* A weak reference the host still holds keeps the control block alive
 * through obs_shutdown(), and releasing it afterwards frees it (#3364). */
static void test_weak_ref_held_across_shutdown_frees_on_release(void **state)
{
	(void)state;
	long before = bnum_allocs();
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		skip();
	}

	struct obs_source_info input = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = input_name,
		.create = input_create,
		.destroy = input_destroy,
	};
	obs_register_source(&input);

	obs_source_t *source = obs_source_create(INPUT_ID, "left alive", NULL, NULL);
	assert_non_null(source);
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	assert_non_null(weak);

	obs_shutdown();

	/* the control block, for the weak reference */
	assert_int_equal(bnum_allocs() - before, 1);
	obs_weak_source_release(weak);
	assert_int_equal(bnum_allocs() - before, 0);
}

/* Sources a scene in the main canvas still holds are released by the
 * scene's deferred destroy, not left alive (#3441).
 *
 * obs_free_data() releases the main canvas, which queues the scene's destroy
 * on the destruction task thread. The scene's inputs stay in the source table
 * until that destroy releases its items. When the left-alive loop reached
 * them first, it destroyed them and freed their control blocks, and the
 * scene's teardown then released the freed blocks (use-after-free under
 * ASan). The delay in the first item_remove signal, on the destruction
 * thread before the items are released, forces that order; it synchronizes
 * nothing. Without ASan the use-after-free is invisible, so the case checks
 * the log line of the left-alive loop instead. */
static bool left_alive_logged;
static log_handler_t previous_log_handler;
static void *previous_log_param;

static void record_left_alive(int lvl, const char *msg, va_list args, void *param)
{
	(void)param;
	if (strstr(msg, "source(s) were remaining"))
		left_alive_logged = true;
	if (previous_log_handler)
		previous_log_handler(lvl, msg, args, previous_log_param);
}

static void delay_first_item_remove(void *data, calldata_t *cd)
{
	(void)cd;
	bool *delayed = data;
	if (!*delayed) {
		*delayed = true;
		os_sleep_ms(200);
	}
}

static void test_scene_items_are_released_not_left_alive(void **state)
{
	(void)state;
	long before = bnum_allocs();
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		skip();
	}

	struct obs_source_info input = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = input_name,
		.create = input_create,
		.destroy = input_destroy,
	};
	obs_register_source(&input);

	obs_scene_t *scene = obs_scene_create("held by the main canvas");
	obs_source_t *first = obs_source_create(INPUT_ID, "first item", NULL, NULL);
	obs_source_t *second = obs_source_create(INPUT_ID, "second item", NULL, NULL);
	assert_non_null(scene);
	assert_non_null(first);
	assert_non_null(second);
	assert_non_null(obs_scene_add(scene, first));
	assert_non_null(obs_scene_add(scene, second));
	obs_set_output_source(0, obs_scene_get_source(scene));

	static bool delayed;
	delayed = false;
	signal_handler_connect(obs_source_get_signal_handler(obs_scene_get_source(scene)), "item_remove",
			       delay_first_item_remove, &delayed);

	/* Only the scene holds the inputs, and only the main canvas the scene. */
	obs_source_release(first);
	obs_source_release(second);
	obs_scene_release(scene);

	left_alive_logged = false;
	base_get_log_handler(&previous_log_handler, &previous_log_param);
	base_set_log_handler(record_left_alive, NULL);
	obs_shutdown();
	base_set_log_handler(previous_log_handler, previous_log_param);

	assert_true(delayed);
	assert_false(left_alive_logged);
	assert_int_equal(bnum_allocs() - before, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_sources_left_alive_free_their_canvas_ref),
		cmocka_unit_test(test_weak_ref_held_across_shutdown_frees_on_release),
		cmocka_unit_test(test_scene_items_are_released_not_left_alive),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

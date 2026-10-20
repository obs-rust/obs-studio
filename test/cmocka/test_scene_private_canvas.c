/* Private scenes have no canvas (upstream obsproject/obs-studio PR #12824).
 *
 * Scenes belong to a canvas since OBS 31. obs_scene_create_private() and
 * obs_scene_duplicate() with OBS_SCENE_DUP_PRIVATE_REFS or
 * OBS_SCENE_DUP_PRIVATE_COPY (Studio Mode's "Duplicate Scene") created the
 * scene through obs_source_create_private(), which takes no canvas: libobs
 * logged "Attempted to add Scene without specifying a canvas! Using default
 * canvas instead." on every Studio Mode transition, and the private scene's
 * canvas stayed NULL. A private copy of a scene on another canvas (a plugin's
 * second canvas, e.g. a vertical one) was set up against the main canvas.
 *
 * The cases create private scenes and private duplicates on the main canvas
 * and on a second canvas, and check the scene's canvas and the log. A private
 * scene still stays out of its canvas's list of scenes (it has no name
 * there), so renaming or removing it must not touch that list. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>

#define CANVAS_CX 1280
#define CANVAS_CY 720

static bool obs_up;
static int no_canvas_warnings;

static void count_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	char buf[1024];
	vsnprintf(buf, sizeof(buf), msg, args);
	if (strstr(buf, "without specifying a canvas"))
		no_canvas_warnings++;
	if (lvl <= LOG_WARNING || strstr(buf, "memory leaks"))
		fprintf(stderr, "%s\n", buf);
}

static struct obs_video_info video_info(void)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = CANVAS_CX,
		.base_height = CANVAS_CY,
		.output_width = CANVAS_CX,
		.output_height = CANVAS_CY,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	return ovi;
}

/* The canvas the scene belongs to, compared with expected (both released). */
static void assert_scene_canvas(obs_scene_t *scene, obs_canvas_t *expected)
{
	obs_canvas_t *canvas = obs_source_get_canvas(obs_scene_get_source(scene));
	if (canvas != expected)
		fprintf(stderr, "scene '%s' is on canvas '%s', expected '%s'\n",
			obs_source_get_name(obs_scene_get_source(scene)),
			canvas ? obs_canvas_get_name(canvas) : "(none)", obs_canvas_get_name(expected));
	obs_canvas_release(canvas);
	assert_ptr_equal(canvas, expected);
}

static void private_scene_is_on_main_canvas(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_canvas_t *main_canvas = obs_get_main_canvas();
	no_canvas_warnings = 0;

	obs_scene_t *scene = obs_scene_create_private("private scene");
	assert_non_null(scene);
	assert_scene_canvas(scene, main_canvas);
	assert_int_equal(no_canvas_warnings, 0);

	obs_scene_release(scene);
	obs_canvas_release(main_canvas);
}

static void check_private_duplicate(obs_canvas_t *canvas, enum obs_scene_duplicate_type type)
{
	obs_scene_t *scene = obs_canvas_scene_create(canvas, "original");
	assert_non_null(scene);
	assert_scene_canvas(scene, canvas);

	no_canvas_warnings = 0;
	obs_scene_t *dup = obs_scene_duplicate(scene, "original", type);
	assert_non_null(dup);
	assert_true(obs_obj_is_private(obs_scene_get_source(dup)));
	assert_scene_canvas(dup, canvas);
	assert_int_equal(no_canvas_warnings, 0);

	obs_scene_release(dup);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_wait_for_destroy_queue();
}

/* Studio Mode's program view with "Duplicate Scene" on. */
static void private_duplicate_keeps_main_canvas(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_canvas_t *main_canvas = obs_get_main_canvas();
	check_private_duplicate(main_canvas, OBS_SCENE_DUP_PRIVATE_REFS);
	check_private_duplicate(main_canvas, OBS_SCENE_DUP_PRIVATE_COPY);
	obs_canvas_release(main_canvas);
}

/* The same on a plugin's second canvas: the copy stays on that canvas. */
static void private_duplicate_keeps_extra_canvas(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct obs_video_info ovi = video_info();
	ovi.base_width = 720;
	ovi.base_height = 1280;
	ovi.output_width = 720;
	ovi.output_height = 1280;
	obs_canvas_t *extra = obs_canvas_create("vertical", &ovi, PROGRAM);
	assert_non_null(extra);

	check_private_duplicate(extra, OBS_SCENE_DUP_PRIVATE_REFS);
	check_private_duplicate(extra, OBS_SCENE_DUP_PRIVATE_COPY);

	obs_canvas_remove(extra);
	obs_canvas_release(extra);
	obs_wait_for_destroy_queue();
}

/* Control: a public duplicate ("Duplicate" in the scene list) was always on
 * the original's canvas. */
static void public_duplicate_keeps_canvas(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_canvas_t *main_canvas = obs_get_main_canvas();
	obs_scene_t *scene = obs_canvas_scene_create(main_canvas, "public original");
	obs_scene_t *dup = obs_scene_duplicate(scene, "public copy", OBS_SCENE_DUP_REFS);
	assert_non_null(dup);
	assert_scene_canvas(dup, main_canvas);

	obs_source_remove(obs_scene_get_source(dup));
	obs_scene_release(dup);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_canvas_release(main_canvas);
	obs_wait_for_destroy_queue();
}

/* A private scene is not in its canvas's list of scenes: renaming it keeps
 * that list as it was, and the scenes in it are still found by name. */
static void private_scene_rename_keeps_canvas_scenes(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_canvas_t *main_canvas = obs_get_main_canvas();
	obs_scene_t *a = obs_canvas_scene_create(main_canvas, "scene A");
	obs_scene_t *b = obs_canvas_scene_create(main_canvas, "scene B");
	obs_scene_t *dup = obs_scene_duplicate(a, "scene A", OBS_SCENE_DUP_PRIVATE_REFS);
	assert_non_null(dup);

	obs_source_set_name(obs_scene_get_source(dup), "scene A (program)");

	obs_source_t *found_a = obs_canvas_get_source_by_name(main_canvas, "scene A");
	obs_source_t *found_b = obs_canvas_get_source_by_name(main_canvas, "scene B");
	obs_source_t *found_dup = obs_canvas_get_source_by_name(main_canvas, "scene A (program)");
	assert_ptr_equal(found_a, obs_scene_get_source(a));
	assert_ptr_equal(found_b, obs_scene_get_source(b));
	assert_null(found_dup);
	obs_source_release(found_a);
	obs_source_release(found_b);

	obs_scene_release(dup);
	obs_source_remove(obs_scene_get_source(a));
	obs_scene_release(a);
	obs_source_remove(obs_scene_get_source(b));
	obs_scene_release(b);
	obs_canvas_release(main_canvas);
	obs_wait_for_destroy_queue();
}

/* Removing a private duplicate leaves its canvas's list of scenes alone. */
static void private_scene_remove_keeps_canvas_scenes(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_canvas_t *main_canvas = obs_get_main_canvas();
	obs_scene_t *a = obs_canvas_scene_create(main_canvas, "scene C");
	obs_scene_t *dup = obs_scene_duplicate(a, "scene C", OBS_SCENE_DUP_PRIVATE_REFS);
	assert_non_null(dup);

	obs_source_remove(obs_scene_get_source(dup));
	obs_scene_release(dup);
	obs_wait_for_destroy_queue();

	obs_source_t *found = obs_canvas_get_source_by_name(main_canvas, "scene C");
	assert_ptr_equal(found, obs_scene_get_source(a));
	assert_false(obs_source_removed(found));
	obs_source_release(found);

	obs_source_remove(obs_scene_get_source(a));
	obs_scene_release(a);
	obs_canvas_release(main_canvas);
	obs_wait_for_destroy_queue();
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(count_log, NULL);
	/* obs_startup() needs a display on Linux (run under xvfb-run); the
	 * cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = video_info();
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
	}
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(private_scene_is_on_main_canvas),
		cmocka_unit_test(private_duplicate_keeps_main_canvas),
		cmocka_unit_test(private_duplicate_keeps_extra_canvas),
		cmocka_unit_test(public_duplicate_keeps_canvas),
		cmocka_unit_test(private_scene_rename_keeps_canvas_scenes),
		cmocka_unit_test(private_scene_remove_keeps_canvas_scenes),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}

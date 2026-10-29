/* obs_scene_duplicate() with a source used by several items copies it once:
 * a later item reuses the earlier item's copy. Signals are sent while the
 * copy is built, so an "item_add" handler can remove an earlier item from the
 * new scene; its copy must stay alive until obs_scene_duplicate() returns,
 * or a later item of the same source would take a reference to a freed
 * source.
 *
 * The case copies a scene of boxes A, B and A (public copy, so the new scene
 * signals "source_create" and the test can watch it). When the copy of B is
 * added, the handler removes the copy of A and checks, through a weak
 * reference, that something still holds it. It runs in a child
 * process with its own libobs, so a crash fails the case instead of the whole
 * test. obs_startup() needs a display on Linux, so the case skips without one
 * (run under xvfb-run). */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include <obs.h>
#include <util/base.h>

#define BOX_ID "scene_dup_copy_box"
#define CHILD_SKIP 77
#define CHILD_TIMEOUT 30

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)lvl;
	(void)msg;
	(void)args;
	(void)p;
}

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static void *box_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void box_destroy(void *data)
{
	(void)data;
}

static uint32_t box_size(void *data)
{
	(void)data;
	return 64;
}

static bool start_obs(void)
{
	base_set_log_handler(quiet_log, NULL);
	if (!obs_startup("en-US", NULL, NULL))
		return false;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 640,
		.base_height = 360,
		.output_width = 640,
		.output_height = 360,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS)
		return false;

	struct obs_source_info info = {
		.id = BOX_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = box_name,
		.create = box_create,
		.destroy = box_destroy,
		.get_width = box_size,
		.get_height = box_size,
	};
	obs_register_source(&info);
	return true;
}

static obs_source_t *make_box(const char *name)
{
	return obs_source_create(BOX_ID, name, NULL, NULL);
}

struct watch {
	int adds;
	obs_sceneitem_t *first;
	obs_weak_source_t *first_copy;
	bool removed;
	bool alive_after_remove;
};

static struct watch watch;

/* The first item added is the copy of A, the second the copy of B: remove
 * the copy of A then, and see whether anything still holds it. */
static void item_added(void *param, calldata_t *cd)
{
	(void)param;
	obs_sceneitem_t *item = calldata_ptr(cd, "item");
	watch.adds++;
	if (watch.adds == 1) {
		watch.first = item;
		watch.first_copy = obs_source_get_weak_source(obs_sceneitem_get_source(item));
	} else if (watch.adds == 2 && watch.first) {
		obs_sceneitem_remove(watch.first);
		watch.removed = true;
		obs_source_t *copy = obs_weak_source_get_source(watch.first_copy);
		watch.alive_after_remove = copy != NULL;
		obs_source_release(copy);
	}
}

static void source_created(void *param, calldata_t *cd)
{
	(void)param;
	obs_source_t *source = calldata_ptr(cd, "source");
	if (obs_scene_from_source(source) && strcmp(obs_source_get_name(source), "S copy") == 0)
		signal_handler_connect(obs_source_get_signal_handler(source), "item_add", item_added, NULL);
}

static bool count_items(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	(void)scene;
	size_t *count = param;
	if (obs_source_get_name(obs_sceneitem_get_source(item)))
		(*count)++;
	return true;
}

static int scenario_removed_copy_outlives_duplicate(void)
{
	obs_scene_t *scene = obs_scene_create("S");
	obs_source_t *a = make_box("A");
	obs_source_t *b = make_box("B");
	obs_scene_add(scene, a);
	obs_scene_add(scene, b);
	obs_scene_add(scene, a);
	int ret = 0;

	signal_handler_t *global = obs_get_signal_handler();
	signal_handler_connect(global, "source_create", source_created, NULL);
	obs_scene_t *dup = obs_scene_duplicate(scene, "S copy", OBS_SCENE_DUP_COPY);
	signal_handler_disconnect(global, "source_create", source_created, NULL);

	if (!dup || !watch.removed) {
		fprintf(stderr, "the copy of B was not added after the copy of A (%d adds)\n", watch.adds);
		ret = 1;
	} else if (!watch.alive_after_remove) {
		fprintf(stderr, "the removed copy of A was released while obs_scene_duplicate() still used it\n");
		ret = 1;
	}

	/* every item of the copy still has a live source */
	size_t count = 0;
	if (dup)
		obs_scene_enum_items(dup, count_items, &count);
	if (dup && count != 2) {
		fprintf(stderr, "the copy has %zu items, expected 2\n", count);
		ret = 1;
	}

	obs_weak_source_release(watch.first_copy);
	obs_scene_release(dup);
	obs_source_release(a);
	obs_source_release(b);
	obs_scene_release(scene);
	return ret;
}

typedef int (*scenario_fn)(void);

static void run_in_child(scenario_fn fn, const char *what)
{
	fflush(stderr);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		alarm(CHILD_TIMEOUT);
		if (!start_obs())
			_exit(CHILD_SKIP);
		int ret = fn();
		fflush(stderr);
		_exit(ret);
	}

	int status = 0;
	assert_int_equal(waitpid(pid, &status, 0), pid);
	if (WIFEXITED(status) && WEXITSTATUS(status) == CHILD_SKIP)
		skip();
	if (WIFSIGNALED(status)) {
		fprintf(stderr, "%s: the child died from signal %d (%s)\n", what, WTERMSIG(status),
			WTERMSIG(status) == SIGALRM ? "hang" : "crash");
		fail();
	}
	if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
		fprintf(stderr, "%s failed\n", what);
		fail();
	}
}

static void removed_copy_outlives_duplicate(void **state)
{
	(void)state;
	run_in_child(scenario_removed_copy_outlives_duplicate, "removed copy outlives the duplicate");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(removed_copy_outlives_duplicate),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

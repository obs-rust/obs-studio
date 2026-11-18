/* Duplicating a scene that holds a group copies the group (a group can only
 * sit in one scene), but a public copy was given no name, so the new group
 * showed up in the Sources list as "__unnamed0000", "__unnamed0001", ...
 * (obsproject/obs-studio#14145; since upstream 8126df7cd in 33.0). The copy
 * is named after the group it copies; libobs makes a taken name unique, so a
 * copy of "G" is "G 2". A private copy keeps the name exactly, as before.
 *
 * Each case runs in a child process with its own libobs, so a crash fails the
 * case instead of the whole test. obs_startup() needs a display on Linux, so
 * the cases skip without one (run under xvfb-run). */

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

#define CHILD_SKIP 77
#define CHILD_TIMEOUT 30

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)lvl;
	(void)msg;
	(void)args;
	(void)p;
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
	return obs_reset_video(&ovi) == OBS_VIDEO_SUCCESS;
}

static bool find_group(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	(void)scene;
	if (obs_sceneitem_is_group(item))
		*(obs_sceneitem_t **)param = item;
	return true;
}

static const char *group_name(obs_scene_t *scene)
{
	obs_sceneitem_t *group = NULL;
	obs_scene_enum_items(scene, find_group, &group);
	return group ? obs_source_get_name(obs_sceneitem_get_source(group)) : NULL;
}

/* Scene "S" holds group "G"; duplicate it with `type` and check the name of
 * the copied group (want), that the original keeps "G", and that a public
 * copy can be found by its name. */
static int check_name(enum obs_scene_duplicate_type type, const char *want, const char *what)
{
	obs_scene_t *scene = obs_scene_create("S");
	obs_sceneitem_t *group = obs_scene_add_group(scene, "G");
	int ret = 0;

	bool make_private = type == OBS_SCENE_DUP_PRIVATE_REFS || type == OBS_SCENE_DUP_PRIVATE_COPY;
	obs_scene_t *dup = obs_scene_duplicate(scene, make_private ? "S" : "S copy", type);
	obs_sceneitem_t *copy = NULL;
	obs_scene_enum_items(dup, find_group, &copy);
	if (!copy) {
		fprintf(stderr, "%s: the copy lacks the group\n", what);
		ret = 1;
		goto done;
	}

	obs_source_t *copy_source = obs_sceneitem_get_source(copy);
	const char *got = obs_source_get_name(copy_source);
	if (!got || strcmp(got, want) != 0) {
		fprintf(stderr, "%s: the copied group is named '%s', expected '%s'\n", what, got ? got : "(null)",
			want);
		ret = 1;
	}
	const char *orig = obs_source_get_name(obs_sceneitem_get_source(group));
	if (!orig || strcmp(orig, "G") != 0) {
		fprintf(stderr, "%s: the original group was renamed to '%s'\n", what, orig ? orig : "(null)");
		ret = 1;
	}
	if (!make_private) {
		obs_source_t *by_name = obs_get_source_by_name(want);
		if (by_name != copy_source) {
			fprintf(stderr, "%s: looking up '%s' does not find the copied group\n", what, want);
			ret = 1;
		}
		obs_source_release(by_name);
	}

done:
	obs_scene_release(dup);
	obs_scene_release(scene);
	return ret;
}

static int scenario_refs(void)
{
	return check_name(OBS_SCENE_DUP_REFS, "G 2", "OBS_SCENE_DUP_REFS");
}

static int scenario_copy(void)
{
	return check_name(OBS_SCENE_DUP_COPY, "G 2", "OBS_SCENE_DUP_COPY");
}

static int scenario_private_copy(void)
{
	return check_name(OBS_SCENE_DUP_PRIVATE_COPY, "G", "OBS_SCENE_DUP_PRIVATE_COPY");
}

/* A second public duplicate takes the next free name. */
static int scenario_twice(void)
{
	obs_scene_t *scene = obs_scene_create("S");
	obs_scene_add_group(scene, "G");
	int ret = 0;

	obs_scene_t *first = obs_scene_duplicate(scene, "S copy", OBS_SCENE_DUP_REFS);
	obs_scene_t *second = obs_scene_duplicate(scene, "S copy 2", OBS_SCENE_DUP_REFS);
	const char *na = group_name(first);
	const char *nb = group_name(second);
	if (!na || !nb || strcmp(na, "G 2") != 0 || strcmp(nb, "G 3") != 0) {
		fprintf(stderr, "twice: the copied groups are named '%s' and '%s', expected 'G 2' and 'G 3'\n",
			na ? na : "(null)", nb ? nb : "(null)");
		ret = 1;
	}

	obs_scene_release(second);
	obs_scene_release(first);
	obs_scene_release(scene);
	return ret;
}

/* Child exit codes: 0 ok, 1 failed (message on stderr), CHILD_SKIP. */
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

static void refs_names_copied_group(void **state)
{
	(void)state;
	run_in_child(scenario_refs, "refs");
}

static void copy_names_copied_group(void **state)
{
	(void)state;
	run_in_child(scenario_copy, "public copy");
}

static void private_copy_keeps_group_name(void **state)
{
	(void)state;
	run_in_child(scenario_private_copy, "private copy");
}

static void second_duplicate_takes_next_name(void **state)
{
	(void)state;
	run_in_child(scenario_twice, "twice");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(refs_names_copied_group),
		cmocka_unit_test(copy_names_copied_group),
		cmocka_unit_test(private_copy_keeps_group_name),
		cmocka_unit_test(second_duplicate_takes_next_name),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

/* obs_scene_insert_group() with a non-NULL item list and a count of 0
 * (obs-rust/obs-studio#2945). It took the item to insert the group after as
 * items[count - 1] before it checked for an empty list, so count - 1 wrapped
 * to SIZE_MAX and the read landed on the pointer just before the list. The
 * group was then inserted after whatever that word held.
 *
 * The scene holds A at the bottom and B on top. The empty list is passed as
 * &list[1], with A in list[0], so the wrapped read gives A on every build:
 * the group then lands between A and B instead of on top, as an empty list
 * (like NULL, 0) should leave it.
 *
 * Each case runs in a child process with its own libobs, so a crash fails the
 * case instead of the whole test. Core video is not needed. obs_startup()
 * needs a display on Linux, so the cases skip without one (run under
 * xvfb-run). */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

#include <obs.h>
#include <util/base.h>

#define BOX_ID "group_insert_empty_box"
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

/* A scene with box A at the bottom and box B on top. */
struct layout {
	obs_scene_t *scene;
	obs_sceneitem_t *a;
	obs_sceneitem_t *b;
};

static obs_sceneitem_t *add_box(obs_scene_t *scene, const char *name)
{
	obs_source_t *box = obs_source_create(BOX_ID, name, NULL, NULL);
	obs_sceneitem_t *item = obs_scene_add(scene, box);
	obs_source_release(box);
	return item;
}

static void build_layout(struct layout *l)
{
	l->scene = obs_scene_create("scene");
	l->a = add_box(l->scene, "A");
	l->b = add_box(l->scene, "B");
}

static int check_position(obs_sceneitem_t *item, int want, const char *what)
{
	int got = obs_sceneitem_get_order_position(item);
	if (got != want) {
		fprintf(stderr, "%s is at order position %d, expected %d\n", what, got, want);
		return 1;
	}
	return 0;
}

/* An empty group on top, with A and B where they were. */
static int check_empty_group_on_top(struct layout *l, obs_sceneitem_t *group, const char *what)
{
	if (!group) {
		fprintf(stderr, "%s: no group was added\n", what);
		return 1;
	}
	int failed = 0;
	failed |= check_position(l->a, 0, "A");
	failed |= check_position(l->b, 1, "B");
	failed |= check_position(group, 2, "the group");
	if (obs_sceneitem_get_scene(l->a) != l->scene || obs_sceneitem_get_scene(l->b) != l->scene) {
		fprintf(stderr, "%s moved an item out of the scene\n", what);
		failed = 1;
	}
	if (failed)
		fprintf(stderr, "%s did not add an empty group on top\n", what);
	return failed;
}

static int scenario_empty_list(void)
{
	struct layout l;
	build_layout(&l);
	/* list[0] stands where the wrapped items[count - 1] reads */
	obs_sceneitem_t *list[2] = {l.a, NULL};
	obs_sceneitem_t *group = obs_scene_insert_group(l.scene, "group", &list[1], 0);
	return check_empty_group_on_top(&l, group, "obs_scene_insert_group(items, 0)");
}

static int scenario_empty_list_signal(void)
{
	struct layout l;
	build_layout(&l);
	obs_sceneitem_t *list[2] = {l.a, NULL};
	obs_sceneitem_t *group = obs_scene_insert_group2(l.scene, "group", &list[1], 0, true);
	return check_empty_group_on_top(&l, group, "obs_scene_insert_group2(items, 0)");
}

/* Guard: NULL, 0 (obs_scene_add_group) adds an empty group on top. */
static int scenario_null_list(void)
{
	struct layout l;
	build_layout(&l);
	obs_sceneitem_t *group = obs_scene_insert_group(l.scene, "group", NULL, 0);
	return check_empty_group_on_top(&l, group, "obs_scene_insert_group(NULL, 0)");
}

/* Guard: a one-item list still moves that item into the group, which takes
 * its place in the scene. */
static int scenario_one_item(void)
{
	struct layout l;
	build_layout(&l);
	obs_sceneitem_t *items[] = {l.a};
	obs_sceneitem_t *group = obs_scene_insert_group(l.scene, "group", items, 1);
	if (!group) {
		fprintf(stderr, "obs_scene_insert_group(items, 1) added no group\n");
		return 1;
	}
	if (obs_sceneitem_get_scene(l.a) != obs_sceneitem_group_get_scene(group)) {
		fprintf(stderr, "A was not moved into the group\n");
		return 1;
	}
	return check_position(group, 0, "the group") | check_position(l.b, 1, "B");
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

static void empty_list_adds_group_on_top(void **state)
{
	(void)state;
	run_in_child(scenario_empty_list, "obs_scene_insert_group with an empty list");
}

static void empty_list_signal_adds_group_on_top(void **state)
{
	(void)state;
	run_in_child(scenario_empty_list_signal, "obs_scene_insert_group2 with an empty list");
}

static void null_list_adds_group_on_top(void **state)
{
	(void)state;
	run_in_child(scenario_null_list, "obs_scene_insert_group with NULL");
}

static void one_item_moves_into_group(void **state)
{
	(void)state;
	run_in_child(scenario_one_item, "obs_scene_insert_group with one item");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(empty_list_adds_group_on_top),
		cmocka_unit_test(empty_list_signal_adds_group_on_top),
		cmocka_unit_test(null_list_adds_group_on_top),
		cmocka_unit_test(one_item_moves_into_group),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

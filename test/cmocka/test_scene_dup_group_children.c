/* Studio Mode with "Duplicate Sources" shows a private copy of the scene in
 * Program (obs_scene_duplicate(..., OBS_SCENE_DUP_PRIVATE_COPY)), so editing
 * a source in Preview leaves Program alone until the next transition. Sources
 * inside a group were not copied: the group was, but its items still pointed
 * at the original sources, so editing a grouped text or image in Preview
 * changed Program at once (obsproject/obs-studio#4309, reported by iesv;
 * #6977, reported by corche).
 *
 * The cases check which sources the copy of a scene holding a group points
 * at, for each duplicate type. Each case runs in a child process with its own
 * libobs, so a crash fails the case instead of the whole test. obs_startup()
 * needs a display on Linux, so the cases skip without one (run under
 * xvfb-run). */

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

#define BOX_ID "scene_dup_group_box"
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

/* Scene "S" holds group "G" (which holds box "in group") and box "plain". */
struct layout {
	obs_scene_t *scene;
	obs_source_t *in_group;
	obs_source_t *plain;
};

static obs_source_t *make_box(const char *name)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "text", name);
	obs_source_t *box = obs_source_create(BOX_ID, name, settings, NULL);
	obs_data_release(settings);
	return box;
}

static void build_layout(struct layout *l)
{
	l->scene = obs_scene_create("S");
	obs_sceneitem_t *group = obs_scene_add_group(l->scene, "G");
	l->in_group = make_box("in group");
	l->plain = make_box("plain");
	obs_sceneitem_t *item = obs_scene_add(l->scene, l->in_group);
	obs_sceneitem_group_add_item(group, item);
	obs_scene_add(l->scene, l->plain);
}

static void free_layout(struct layout *l)
{
	obs_source_release(l->in_group);
	obs_source_release(l->plain);
	obs_scene_release(l->scene);
}

struct found {
	obs_sceneitem_t *group;
	obs_source_t *plain;
};

static bool find_items(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	(void)scene;
	struct found *f = param;
	if (obs_sceneitem_is_group(item))
		f->group = item;
	else
		f->plain = obs_sceneitem_get_source(item);
	return true;
}

static obs_source_t *group_child(obs_sceneitem_t *group)
{
	obs_scene_t *g = obs_sceneitem_group_get_scene(group);
	obs_sceneitem_t *child = g ? obs_scene_find_source(g, "in group") : NULL;
	return child ? obs_sceneitem_get_source(child) : NULL;
}

/* Duplicates the layout's scene with `type` and checks whether the plain box
 * and the box in the group were copied (expect_copy) or shared. */
static int check_duplicate(enum obs_scene_duplicate_type type, bool expect_copy, const char *what)
{
	struct layout l;
	build_layout(&l);
	int ret = 0;

	bool make_private = type == OBS_SCENE_DUP_PRIVATE_REFS || type == OBS_SCENE_DUP_PRIVATE_COPY;
	obs_scene_t *dup = obs_scene_duplicate(l.scene, make_private ? "S" : "S copy", type);
	struct found f = {0};
	obs_scene_enum_items(dup, find_items, &f);

	obs_source_t *child = f.group ? group_child(f.group) : NULL;
	if (!f.group || !child || !f.plain) {
		fprintf(stderr, "%s: the copy lacks the group, its box or the plain box\n", what);
		ret = 1;
		goto done;
	}
	if ((f.plain != l.plain) != expect_copy) {
		fprintf(stderr, "%s: the plain box was %s\n", what, expect_copy ? "shared, not copied" : "copied");
		ret = 1;
	}
	if ((child != l.in_group) != expect_copy) {
		fprintf(stderr, "%s: the box in the group was %s\n", what,
			expect_copy ? "shared with the original scene, not copied" : "copied");
		ret = 1;
	}
	if (expect_copy && child != l.in_group) {
		obs_data_t *s = obs_source_get_settings(child);
		if (strcmp(obs_data_get_string(s, "text"), "in group") != 0) {
			fprintf(stderr, "%s: the copied box lost its settings\n", what);
			ret = 1;
		}
		obs_data_release(s);
		if (make_private && !obs_obj_is_private(child)) {
			fprintf(stderr, "%s: the copied box is not private\n", what);
			ret = 1;
		}
	}

done:
	obs_scene_release(dup);
	free_layout(&l);
	return ret;
}

static int scenario_private_copy(void)
{
	return check_duplicate(OBS_SCENE_DUP_PRIVATE_COPY, true, "OBS_SCENE_DUP_PRIVATE_COPY");
}

static int scenario_private_refs(void)
{
	return check_duplicate(OBS_SCENE_DUP_PRIVATE_REFS, false, "OBS_SCENE_DUP_PRIVATE_REFS");
}

static int scenario_refs(void)
{
	return check_duplicate(OBS_SCENE_DUP_REFS, false, "OBS_SCENE_DUP_REFS");
}

/* A public copy keeps sharing a group's sources (unchanged): only the plain
 * box is copied. */
static int scenario_copy(void)
{
	struct layout l;
	build_layout(&l);
	int ret = 0;

	obs_scene_t *dup = obs_scene_duplicate(l.scene, "S copy", OBS_SCENE_DUP_COPY);
	struct found f = {0};
	obs_scene_enum_items(dup, find_items, &f);
	obs_source_t *child = f.group ? group_child(f.group) : NULL;
	if (!child || !f.plain || f.plain == l.plain || child != l.in_group) {
		fprintf(stderr, "OBS_SCENE_DUP_COPY: expected a copied plain box and a shared box in the group\n");
		ret = 1;
	}

	obs_scene_release(dup);
	free_layout(&l);
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

static void private_copy_copies_group_children(void **state)
{
	(void)state;
	run_in_child(scenario_private_copy, "private copy");
}

static void private_refs_shares_group_children(void **state)
{
	(void)state;
	run_in_child(scenario_private_refs, "private refs");
}

static void refs_shares_group_children(void **state)
{
	(void)state;
	run_in_child(scenario_refs, "refs");
}

static void copy_shares_group_children(void **state)
{
	(void)state;
	run_in_child(scenario_copy, "public copy");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(private_copy_copies_group_children),
		cmocka_unit_test(private_refs_shares_group_children),
		cmocka_unit_test(refs_shares_group_children),
		cmocka_unit_test(copy_shares_group_children),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

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

static bool vec2_eq(const struct vec2 *a, const struct vec2 *b)
{
	return a->x == b->x && a->y == b->y;
}

static bool collect_items(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	(void)scene;
	obs_sceneitem_t **out = param;
	for (size_t i = 0; i < 3; i++) {
		if (!out[i]) {
			out[i] = item;
			break;
		}
	}
	return true;
}

/* The group's own transform and bounds, and its items' order, visibility,
 * crop and show/hide transitions, carry over to the private copy. */
static int scenario_private_copy_keeps_group_state(void)
{
	struct layout l;
	build_layout(&l);
	int ret = 0;

	obs_sceneitem_t *group = obs_scene_get_group(l.scene, "G");
	obs_source_t *second = make_box("second");
	obs_sceneitem_t *second_item = obs_scene_add(l.scene, second);
	obs_sceneitem_group_add_item(group, second_item);
	obs_source_t *third = make_box("third");
	obs_sceneitem_t *third_item = obs_scene_add(l.scene, third);
	obs_sceneitem_group_add_item(group, third_item);

	obs_scene_t *gscene = obs_sceneitem_group_get_scene(group);
	obs_sceneitem_t *first_item = obs_scene_find_source(gscene, "in group");
	struct vec2 pos = {40.0f, 30.0f};
	struct vec2 scale = {2.0f, 0.5f};
	struct vec2 bounds = {300.0f, 200.0f};
	struct vec2 child_pos = {100.0f, 10.0f};
	obs_sceneitem_set_pos(first_item, &child_pos);
	obs_sceneitem_set_pos(group, &pos);
	obs_sceneitem_set_scale(group, &scale);
	obs_sceneitem_set_bounds_type(group, OBS_BOUNDS_SCALE_INNER);
	obs_sceneitem_set_bounds(group, &bounds);
	obs_sceneitem_set_visible(second_item, false);
	struct obs_sceneitem_crop crop = {1, 2, 3, 4};
	obs_sceneitem_set_crop(third_item, &crop);
	obs_source_t *show_tr = make_box("show");
	obs_source_t *hide_tr = make_box("hide");
	obs_sceneitem_set_transition(first_item, true, show_tr);
	obs_sceneitem_set_transition(first_item, false, hide_tr);
	obs_sceneitem_set_transition_duration(first_item, true, 250);
	obs_sceneitem_set_transition_duration(first_item, false, 750);

	obs_scene_t *dup = obs_scene_duplicate(l.scene, "S", OBS_SCENE_DUP_PRIVATE_COPY);
	struct found f = {0};
	obs_scene_enum_items(dup, find_items, &f);
	obs_sceneitem_t *items[3] = {0};
	obs_scene_t *dgscene = f.group ? obs_sceneitem_group_get_scene(f.group) : NULL;
	if (dgscene)
		obs_scene_enum_items(dgscene, collect_items, items);
	if (!items[0] || !items[1] || !items[2]) {
		fprintf(stderr, "keeps state: the copied group lacks items\n");
		ret = 1;
		goto done;
	}

	struct obs_transform_info a, b;
	obs_sceneitem_get_info2(group, &a);
	obs_sceneitem_get_info2(f.group, &b);
	if (!vec2_eq(&a.pos, &b.pos) || !vec2_eq(&a.scale, &b.scale) || a.rot != b.rot ||
	    a.bounds_type != b.bounds_type || !vec2_eq(&a.bounds, &b.bounds) || a.alignment != b.alignment) {
		fprintf(stderr, "keeps state: the group's transform or bounds changed\n");
		ret = 1;
	}
	if (obs_source_get_width(obs_sceneitem_get_source(group)) !=
		    obs_source_get_width(obs_sceneitem_get_source(f.group)) ||
	    obs_source_get_height(obs_sceneitem_get_source(group)) !=
		    obs_source_get_height(obs_sceneitem_get_source(f.group))) {
		fprintf(stderr, "keeps state: the group's size changed\n");
		ret = 1;
	}

	obs_sceneitem_t *orig[3] = {0};
	obs_scene_enum_items(gscene, collect_items, orig);
	for (size_t i = 0; i < 3; i++) {
		obs_data_t *want = obs_source_get_settings(obs_sceneitem_get_source(orig[i]));
		obs_data_t *got = obs_source_get_settings(obs_sceneitem_get_source(items[i]));
		if (strcmp(obs_data_get_string(got, "text"), obs_data_get_string(want, "text")) != 0) {
			fprintf(stderr, "keeps state: item %zu is '%s', expected '%s'\n", i,
				obs_data_get_string(got, "text"), obs_data_get_string(want, "text"));
			ret = 1;
		}
		obs_data_release(want);
		obs_data_release(got);
	}

	/* Locate the copies of the three boxes by name, wherever they sit. */
	obs_sceneitem_t *by_name[3] = {0};
	const char *names[3] = {"in group", "second", "third"};
	for (size_t i = 0; i < 3; i++) {
		for (size_t j = 0; j < 3; j++) {
			if (strcmp(obs_source_get_name(obs_sceneitem_get_source(items[j])), names[i]) == 0)
				by_name[i] = items[j];
		}
	}
	if (!by_name[0] || !by_name[1] || !by_name[2]) {
		fprintf(stderr, "keeps state: a copied box lost its name\n");
		ret = 1;
		goto done;
	}
	items[0] = by_name[0];
	items[1] = by_name[1];
	items[2] = by_name[2];

	struct vec2 got_pos;
	obs_sceneitem_get_pos(items[0], &got_pos);
	if (!vec2_eq(&got_pos, &child_pos)) {
		fprintf(stderr, "keeps state: the grouped item moved\n");
		ret = 1;
	}
	if (!obs_sceneitem_visible(items[0]) || obs_sceneitem_visible(items[1]) || !obs_sceneitem_visible(items[2])) {
		fprintf(stderr, "keeps state: item visibility changed\n");
		ret = 1;
	}
	struct obs_sceneitem_crop got_crop;
	obs_sceneitem_get_crop(items[2], &got_crop);
	if (got_crop.left != 1 || got_crop.top != 2 || got_crop.right != 3 || got_crop.bottom != 4) {
		fprintf(stderr, "keeps state: the crop changed\n");
		ret = 1;
	}
	obs_source_t *got_show = obs_sceneitem_get_transition(items[0], true);
	obs_source_t *got_hide = obs_sceneitem_get_transition(items[0], false);
	if (!got_show || !got_hide || obs_sceneitem_get_transition_duration(items[0], true) != 250 ||
	    obs_sceneitem_get_transition_duration(items[0], false) != 750) {
		fprintf(stderr, "keeps state: the show/hide transitions were lost\n");
		ret = 1;
	}

done:
	obs_scene_release(dup);
	obs_source_release(show_tr);
	obs_source_release(hide_tr);
	obs_source_release(second);
	obs_source_release(third);
	free_layout(&l);
	return ret;
}

static bool gone(obs_weak_source_t *weak)
{
	obs_source_t *source = obs_weak_source_get_source(weak);
	obs_source_release(source);
	return source == NULL;
}

/* Releasing the private copy (end of the transition, Studio Mode off) frees
 * the copied group and the copies of its sources, and leaves the originals. */
static int scenario_private_copy_release(void)
{
	struct layout l;
	build_layout(&l);
	int ret = 0;

	obs_scene_t *dup = obs_scene_duplicate(l.scene, "S", OBS_SCENE_DUP_PRIVATE_COPY);
	struct found f = {0};
	obs_scene_enum_items(dup, find_items, &f);
	obs_source_t *child = f.group ? group_child(f.group) : NULL;
	if (!child || child == l.in_group) {
		fprintf(stderr, "release: the box in the group was not copied\n");
		obs_scene_release(dup);
		free_layout(&l);
		return 1;
	}
	obs_weak_source_t *weak_child = obs_source_get_weak_source(child);
	obs_weak_source_t *weak_group = obs_source_get_weak_source(obs_sceneitem_get_source(f.group));

	/* A scene is destroyed on libobs's destroy queue, and its group (then
	 * the group's sources) only when the scene's items go, so wait for
	 * each step. */
	obs_scene_release(dup);
	for (int i = 0; i < 10 && !(gone(weak_child) && gone(weak_group)); i++)
		obs_wait_for_destroy_queue();

	obs_source_t *still = obs_weak_source_get_source(weak_child);
	if (still) {
		fprintf(stderr, "release: the copied box outlived the copy\n");
		obs_source_release(still);
		ret = 1;
	}
	still = obs_weak_source_get_source(weak_group);
	if (still) {
		fprintf(stderr, "release: the copied group outlived the copy\n");
		obs_source_release(still);
		ret = 1;
	}
	obs_weak_source_release(weak_child);
	obs_weak_source_release(weak_group);

	obs_sceneitem_t *orig_group = obs_scene_get_group(l.scene, "G");
	if (!orig_group || group_child(orig_group) != l.in_group || obs_source_removed(l.in_group)) {
		fprintf(stderr, "release: the original group lost its box\n");
		ret = 1;
	}

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

static void private_copy_keeps_group_state(void **state)
{
	(void)state;
	run_in_child(scenario_private_copy_keeps_group_state, "private copy keeps group state");
}

static void private_copy_release_frees_copies(void **state)
{
	(void)state;
	run_in_child(scenario_private_copy_release, "private copy release");
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
		cmocka_unit_test(private_copy_keeps_group_state),
		cmocka_unit_test(private_copy_release_frees_copies),
		cmocka_unit_test(private_refs_shares_group_children),
		cmocka_unit_test(refs_shares_group_children),
		cmocka_unit_test(copy_shares_group_children),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

/* obs_source_enum_full_tree() must walk a root that lists its children only
 * through enum_all_sources.
 *
 * The full-tree walk descends into a child through enum_all_sources, or
 * enum_active_sources when the child has no enum_all_sources. The root used
 * to return early unless it was a transition or had enum_active_sources, so
 * a source type with only enum_all_sources was walked as a child but not as
 * the root. The walk's recursion checks then missed a cycle through such a
 * root: obs_source_add_active_child() (here), and obs_transition_start() and
 * obs_transition_set() on such a destination.
 *
 * The cases register four lister types (only enum_all_sources, only
 * enum_active_sources, both, neither), each listing a leaf, and check what
 * the walk visits and that obs_source_add_active_child() refuses a parent
 * the child already lists. No video is needed. obs_startup() needs a display
 * on Linux, so the test runs under xvfb-run when it is installed and the
 * cases skip without a display.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs.h>
#include <util/base.h>
#include <util/bmem.h>

#define LEAF_ID "test_source_full_tree_root_leaf"
#define ALL_ID "test_source_full_tree_root_all"
#define ACTIVE_ID "test_source_full_tree_root_active"
#define BOTH_ID "test_source_full_tree_root_both"
#define NEITHER_ID "test_source_full_tree_root_neither"
#define MAX_VISITS 8

static bool obs_up;

/* A lister lists `all` through enum_all_sources and `active` through
 * enum_active_sources (each only if its type has that callback). */
struct lister {
	obs_source_t *self;
	obs_source_t *all;
	obs_source_t *active;
};

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)lvl;
	(void)msg;
	(void)args;
	(void)p;
}

static const char *fixture_name(void *type_data)
{
	(void)type_data;
	return "full tree root fixture";
}

static void *leaf_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return (void *)1;
}

static void leaf_destroy(void *data)
{
	(void)data;
}

static uint32_t fixture_size(void *data)
{
	(void)data;
	return 16;
}

static void *lister_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	struct lister *lister = bzalloc(sizeof(*lister));
	lister->self = source;
	return lister;
}

static void lister_destroy(void *data)
{
	bfree(data);
}

static void lister_enum_all(void *data, obs_source_enum_proc_t enum_callback, void *param)
{
	struct lister *lister = data;
	if (lister->all)
		enum_callback(lister->self, lister->all, param);
}

static void lister_enum_active(void *data, obs_source_enum_proc_t enum_callback, void *param)
{
	struct lister *lister = data;
	if (lister->active)
		enum_callback(lister->self, lister->active, param);
}

static void register_lister(const char *id, bool all, bool active)
{
	struct obs_source_info info = {
		.id = id,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = fixture_name,
		.create = lister_create,
		.destroy = lister_destroy,
		.get_width = fixture_size,
		.get_height = fixture_size,
		.enum_all_sources = all ? lister_enum_all : NULL,
		.enum_active_sources = active ? lister_enum_active : NULL,
	};
	obs_register_source(&info);
}

static void register_sources(void)
{
	struct obs_source_info leaf = {
		.id = LEAF_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = fixture_name,
		.create = leaf_create,
		.destroy = leaf_destroy,
		.get_width = fixture_size,
		.get_height = fixture_size,
	};
	obs_register_source(&leaf);

	register_lister(ALL_ID, true, false);
	register_lister(ACTIVE_ID, false, true);
	register_lister(BOTH_ID, true, true);
	register_lister(NEITHER_ID, false, false);
}

struct fixture {
	obs_source_t *leaf;  /* listed through enum_all_sources */
	obs_source_t *other; /* listed through enum_active_sources */
	obs_source_t *spare; /* listed by nothing */
	obs_source_t *lister;
};

static struct fixture *fixture_new(const char *lister_id)
{
	struct fixture *fx = bzalloc(sizeof(*fx));
	fx->leaf = obs_source_create_private(LEAF_ID, "leaf", NULL);
	fx->other = obs_source_create_private(LEAF_ID, "other", NULL);
	fx->spare = obs_source_create_private(LEAF_ID, "spare", NULL);
	fx->lister = obs_source_create_private(lister_id, "lister", NULL);
	assert_non_null(fx->leaf);
	assert_non_null(fx->other);
	assert_non_null(fx->spare);
	assert_non_null(fx->lister);

	struct lister *lister = obs_obj_get_data(fx->lister);
	assert_non_null(lister);
	lister->all = fx->leaf;
	lister->active = fx->other;
	return fx;
}

static void fixture_free(struct fixture *fx)
{
	obs_source_release(fx->lister);
	obs_source_release(fx->spare);
	obs_source_release(fx->other);
	obs_source_release(fx->leaf);
	bfree(fx);
}

struct visits {
	size_t count;
	obs_source_t *parent[MAX_VISITS];
	obs_source_t *child[MAX_VISITS];
};

static void record_visit(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct visits *visits = param;
	if (visits->count < MAX_VISITS) {
		visits->parent[visits->count] = parent;
		visits->child[visits->count] = child;
	}
	visits->count++;
}

/* Walks the lister's full tree and checks it visited exactly `want` (or
 * nothing when `want` is NULL), as a child of the lister. */
static bool full_tree_visits(struct fixture *fx, obs_source_t *want, const char *what)
{
	struct visits visits = {0};
	obs_source_enum_full_tree(fx->lister, record_visit, &visits);

	size_t want_count = want ? 1 : 0;
	if (visits.count == want_count && (!want || (visits.child[0] == want && visits.parent[0] == fx->lister)))
		return true;

	fprintf(stderr, "%s: the full-tree walk visited %zu source(s), expected %zu", what, visits.count, want_count);
	if (visits.count)
		fprintf(stderr, ", first '%s'", obs_source_get_name(visits.child[0]));
	if (want)
		fprintf(stderr, ", expected '%s'", obs_source_get_name(want));
	fprintf(stderr, "\n");
	return false;
}

/* obs_source_add_active_child(parent, lister) must refuse a parent the
 * lister's full tree holds, and accept one it does not. */
static bool add_active_child_is(struct fixture *fx, obs_source_t *parent, bool want, const char *what)
{
	bool added = obs_source_add_active_child(parent, fx->lister);
	if (added)
		obs_source_remove_active_child(parent, fx->lister);
	if (added == want)
		return true;

	fprintf(stderr, "%s: obs_source_add_active_child('%s', lister) returned %s, expected %s\n", what,
		obs_source_get_name(parent), added ? "true" : "false", want ? "true" : "false");
	return false;
}

/* The bug: a root with only enum_all_sources is walked. */
static void all_only_root_is_walked(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	struct fixture *fx = fixture_new(ALL_ID);
	bool ok = full_tree_visits(fx, fx->leaf, "enum_all_sources only");
	fixture_free(fx);
	assert_true(ok);
}

/* What the bug breaks: the recursion check of obs_source_add_active_child. */
static void all_only_root_cycle_is_refused(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	struct fixture *fx = fixture_new(ALL_ID);
	bool ok = add_active_child_is(fx, fx->leaf, false, "enum_all_sources only, listed parent");
	ok &= add_active_child_is(fx, fx->spare, true, "enum_all_sources only, unlisted parent");
	fixture_free(fx);
	assert_true(ok);
}

/* Control: a root with only enum_active_sources is walked through it. */
static void active_only_root_is_walked(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	struct fixture *fx = fixture_new(ACTIVE_ID);
	bool ok = full_tree_visits(fx, fx->other, "enum_active_sources only");
	ok &= add_active_child_is(fx, fx->other, false, "enum_active_sources only, listed parent");
	ok &= add_active_child_is(fx, fx->spare, true, "enum_active_sources only, unlisted parent");
	fixture_free(fx);
	assert_true(ok);
}

/* Control: a root with both callbacks is walked through enum_all_sources
 * alone, as a child is. */
static void both_root_walks_all_sources(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	struct fixture *fx = fixture_new(BOTH_ID);
	bool ok = full_tree_visits(fx, fx->leaf, "both callbacks");
	ok &= add_active_child_is(fx, fx->leaf, false, "both callbacks, listed parent");
	ok &= add_active_child_is(fx, fx->other, true, "both callbacks, active-only parent");
	fixture_free(fx);
	assert_true(ok);
}

/* Control: a root with neither callback visits nothing. */
static void neither_root_visits_nothing(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	struct fixture *fx = fixture_new(NEITHER_ID);
	bool ok = full_tree_visits(fx, NULL, "neither callback");
	ok &= add_active_child_is(fx, fx->leaf, true, "neither callback");
	fixture_free(fx);
	assert_true(ok);
}

static int group_setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);
	/* obs_startup() needs a display on Linux (run under xvfb-run); the
	 * cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}
	register_sources();
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(all_only_root_is_walked),     cmocka_unit_test(all_only_root_cycle_is_refused),
		cmocka_unit_test(active_only_root_is_walked),  cmocka_unit_test(both_root_walks_all_sources),
		cmocka_unit_test(neither_root_visits_nothing),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

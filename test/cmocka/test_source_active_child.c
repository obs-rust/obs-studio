/* Characterization of obs_source_add_active_child() and
 * obs_source_remove_active_child() (#5622): scenes, groups and transitions
 * call them when a child gains or loses a parent that is showing.
 *
 * What it pins, on private sources of a started core, calling the exported
 * functions of the real libobs:
 *
 *   - NULL parent or child: one LOG_DEBUG line,
 *     "<function>: Null 'parent' parameter" (or 'child'; parent is checked
 *     first), and nothing else: add returns false, nothing is walked or
 *     activated.
 *   - add with parent == child: one LOG_WARNING line,
 *     "obs_source_add_active_child: parent == child", false, no walk.
 *     remove has no such check: it deactivates the source itself.
 *   - add walks the child's full tree once (with the parent as the walk's
 *     stop source) and refuses, with nothing activated and nothing logged,
 *     when the parent turns up anywhere in it: as a child of an edge, deeper
 *     down, or as the parent an edge reports. A real cycle (the parent lists
 *     the child, which lists the parent) is refused without walking the
 *     parent. remove never walks.
 *   - Otherwise the child gets show_refs(parent) activations, the first
 *     activate_refs(parent) of them MAIN_VIEW and the rest AUX_VIEW, in that
 *     order (seen through the child's enum_active_sources, which each
 *     activation's tree walk calls once per counter it raises), and add
 *     returns true. remove mirrors it with deactivations, each lowering only
 *     a counter above zero.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * when it is installed. No video is reset, so no tick reacts to the
 * counters. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#define NODE_ID "test_source_active_child_node"
#define MAX_CHILDREN 4
#define MAX_RECORDS 32

/* ------------------------------------------------------------------------- */
/* log capture */

static unsigned log_count;
static int log_level;
static char log_message[160];
#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static log_handler_t saved_handler;
static void *saved_param;

static void capture_start(void)
{
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
}

static void capture_stop(void)
{
	base_set_log_handler(saved_handler, saved_param);
}

/* ------------------------------------------------------------------------- */
/* the node source type */

/* A node lists `children` through enum_all_sources (the full-tree walk's
 * callback), reporting `report_as` as their parent (itself by default), and
 * counts those walks. Its enum_active_sources (the activation tree walk's
 * callback) lists nothing and records the node's own counters each time. */
struct node {
	obs_source_t *self;
	obs_source_t *report_as;
	obs_source_t *children[MAX_CHILDREN];
	size_t child_count;
	unsigned walks;
	size_t records;
	long show[MAX_RECORDS];
	long active[MAX_RECORDS];
};

static const char *node_name(void *type_data)
{
	(void)type_data;
	return "active child node";
}

static void *node_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	struct node *node = bzalloc(sizeof(*node));
	node->self = source;
	node->report_as = source;
	return node;
}

static void node_destroy(void *data)
{
	bfree(data);
}

static uint32_t node_size(void *data)
{
	(void)data;
	return 16;
}

static void node_enum_all(void *data, obs_source_enum_proc_t enum_callback, void *param)
{
	struct node *node = data;
	node->walks++;
	for (size_t i = 0; i < node->child_count; i++)
		enum_callback(node->report_as, node->children[i], param);
}

static void node_enum_active(void *data, obs_source_enum_proc_t enum_callback, void *param)
{
	(void)enum_callback;
	(void)param;
	struct node *node = data;
	if (node->records < MAX_RECORDS) {
		node->show[node->records] = os_atomic_load_long(&node->self->show_refs);
		node->active[node->records] = os_atomic_load_long(&node->self->activate_refs);
	}
	node->records++;
}

static void register_node(void)
{
	struct obs_source_info info = {
		.id = NODE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = node_name,
		.create = node_create,
		.destroy = node_destroy,
		.get_width = node_size,
		.get_height = node_size,
		.enum_all_sources = node_enum_all,
		.enum_active_sources = node_enum_active,
	};
	obs_register_source(&info);
}

/* ------------------------------------------------------------------------- */
/* fixture */

#define NODES 5

struct fixture {
	obs_source_t *source[NODES];
	struct node *node[NODES];
};

static struct fixture fx;

static int fixture_setup(void **state)
{
	(void)state;
	static const char *names[NODES] = {"a", "b", "c", "d", "e"};
	for (size_t i = 0; i < NODES; i++) {
		fx.source[i] = obs_source_create_private(NODE_ID, names[i], NULL);
		assert_non_null(fx.source[i]);
		fx.node[i] = obs_obj_get_data(fx.source[i]);
		assert_non_null(fx.node[i]);
	}
	return 0;
}

static int fixture_teardown(void **state)
{
	(void)state;
	for (size_t i = 0; i < NODES; i++) {
		os_atomic_set_long(&fx.source[i]->show_refs, 0);
		os_atomic_set_long(&fx.source[i]->activate_refs, 0);
		obs_source_release(fx.source[i]);
	}
	/* Sources are destroyed on the destruction thread, which logs. Let it
	 * finish here, so those lines never land in the next test's capture. */
	obs_wait_for_destroy_queue();
	memset(&fx, 0, sizeof(fx));
	return 0;
}

static void list(size_t parent, size_t child)
{
	struct node *node = fx.node[parent];
	assert_true(node->child_count < MAX_CHILDREN);
	node->children[node->child_count++] = fx.source[child];
}

static void set_refs(size_t i, long show, long active)
{
	os_atomic_set_long(&fx.source[i]->show_refs, show);
	os_atomic_set_long(&fx.source[i]->activate_refs, active);
}

static void assert_refs(size_t i, long show, long active)
{
	assert_int_equal(os_atomic_load_long(&fx.source[i]->show_refs), show);
	assert_int_equal(os_atomic_load_long(&fx.source[i]->activate_refs), active);
}

/* The counters node i's enum_active_sources saw, in call order. */
static void assert_records(size_t i, const long (*want)[2], size_t count)
{
	struct node *node = fx.node[i];
	assert_int_equal(node->records, count);
	for (size_t r = 0; r < count; r++) {
		assert_int_equal(node->show[r], want[r][0]);
		assert_int_equal(node->active[r], want[r][1]);
	}
}

static void clear_records(void)
{
	for (size_t i = 0; i < NODES; i++) {
		fx.node[i]->records = 0;
		fx.node[i]->walks = 0;
	}
}

static bool add(size_t parent, size_t child)
{
	return obs_source_add_active_child(fx.source[parent], fx.source[child]);
}

static void remove_child(size_t parent, size_t child)
{
	obs_source_remove_active_child(fx.source[parent], fx.source[child]);
}

/* ------------------------------------------------------------------------- */
/* NULL and self */

static void null_parent_or_child_logs_once(void **state)
{
	(void)state;
	set_refs(0, 2, 1);
	static const struct {
		bool parent, child;
		const char *which;
	} cases[] = {
		{false, true, "parent"},
		{true, false, "child"},
		{false, false, "parent"},
	};
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		obs_source_t *parent = cases[i].parent ? fx.source[0] : NULL;
		obs_source_t *child = cases[i].child ? fx.source[1] : NULL;
		char want[160];

		capture_start();
		bool added = obs_source_add_active_child(parent, child);
		capture_stop();
		snprintf(want, sizeof(want), "obs_source_add_active_child: Null '%s' parameter", cases[i].which);
		assert_false(added);
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		assert_string_equal(log_message, want);

		capture_start();
		obs_source_remove_active_child(parent, child);
		capture_stop();
		snprintf(want, sizeof(want), "obs_source_remove_active_child: Null '%s' parameter", cases[i].which);
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		assert_string_equal(log_message, want);
	}
	for (size_t i = 0; i < NODES; i++) {
		assert_int_equal(fx.node[i]->walks, 0);
		assert_int_equal(fx.node[i]->records, 0);
	}
	assert_refs(0, 2, 1);
	assert_refs(1, 0, 0);
}

static void add_parent_equals_child_warns(void **state)
{
	(void)state;
	set_refs(0, 2, 1);
	list(0, 1);
	capture_start();
	bool added = add(0, 0);
	capture_stop();
	assert_false(added);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_WARNING);
	assert_string_equal(log_message, "obs_source_add_active_child: parent == child");
	assert_int_equal(fx.node[0]->walks, 0);
	assert_int_equal(fx.node[0]->records, 0);
	assert_refs(0, 2, 1);
	assert_refs(1, 0, 0);
}

/* remove has no self check: the source deactivates itself, with the two
 * counts it loaded before the loop (2 and 1: MAIN_VIEW, then AUX_VIEW). */
static void remove_parent_equals_child_deactivates_itself(void **state)
{
	(void)state;
	set_refs(0, 2, 1);
	capture_start();
	remove_child(0, 0);
	capture_stop();
	assert_int_equal(log_count, 0);
	static const long want[][2] = {{1, 1}, {1, 0}, {0, 0}};
	assert_records(0, want, 3);
	assert_int_equal(fx.node[0]->walks, 0);
	assert_refs(0, 0, 0);
}

/* ------------------------------------------------------------------------- */
/* the cycle check */

static void refuse(size_t parent, size_t child)
{
	set_refs(parent, 3, 2);
	clear_records();
	capture_start();
	bool added = add(parent, child);
	capture_stop();
	assert_false(added);
	assert_int_equal(log_count, 0);
	for (size_t i = 0; i < NODES; i++) {
		assert_int_equal(fx.node[i]->records, 0);
		if (i != parent)
			assert_refs(i, 0, 0);
	}
	assert_refs(parent, 3, 2);
}

/* b lists a: adding b under a is refused. */
static void parent_listed_by_child_is_refused(void **state)
{
	(void)state;
	list(1, 0);
	refuse(0, 1);
	assert_int_equal(fx.node[1]->walks, 1);
	assert_int_equal(fx.node[0]->walks, 0);
}

/* b lists c, c lists d, d lists a: refused through the whole tree. */
static void parent_deep_in_child_tree_is_refused(void **state)
{
	(void)state;
	list(1, 2);
	list(2, 3);
	list(3, 0);
	refuse(0, 1);
	assert_int_equal(fx.node[1]->walks, 1);
	assert_int_equal(fx.node[2]->walks, 1);
	assert_int_equal(fx.node[3]->walks, 1);
	assert_int_equal(fx.node[0]->walks, 0);
}

/* c (under b) lists e but reports a as the edge's parent: refused. */
static void parent_reported_as_edge_parent_is_refused(void **state)
{
	(void)state;
	list(1, 2);
	list(2, 4);
	fx.node[2]->report_as = fx.source[0];
	refuse(0, 1);
	assert_int_equal(fx.node[2]->walks, 1);
	assert_int_equal(fx.node[4]->walks, 1);
}

/* a lists b and b lists a: refused, and the walk stops at a (the stop
 * source) instead of descending into it. */
static void real_cycle_is_refused_without_walking_parent(void **state)
{
	(void)state;
	list(0, 1);
	list(1, 0);
	refuse(0, 1);
	assert_int_equal(fx.node[1]->walks, 1);
	assert_int_equal(fx.node[0]->walks, 0);
}

/* b's tree (c, d) does not hold a: accepted, walked once, activated. */
static void unrelated_tree_is_accepted(void **state)
{
	(void)state;
	list(1, 2);
	list(2, 3);
	set_refs(0, 1, 1);
	capture_start();
	bool added = add(0, 1);
	capture_stop();
	assert_true(added);
	assert_int_equal(log_count, 0);
	assert_int_equal(fx.node[1]->walks, 1);
	assert_int_equal(fx.node[2]->walks, 1);
	assert_int_equal(fx.node[3]->walks, 1);
	assert_int_equal(fx.node[0]->walks, 0);
	static const long want[][2] = {{1, 0}, {1, 1}};
	assert_records(1, want, 2);
	assert_refs(1, 1, 1);
	/* The activation walks b's active tree only: c and d are untouched. */
	assert_refs(2, 0, 0);
	assert_refs(3, 0, 0);
}

/* A hidden parent: accepted after the walk, nothing activated. */
static void hidden_parent_accepts_without_activating(void **state)
{
	(void)state;
	set_refs(0, 0, 0);
	capture_start();
	bool added = add(0, 1);
	capture_stop();
	assert_true(added);
	assert_int_equal(log_count, 0);
	assert_int_equal(fx.node[1]->walks, 1);
	assert_int_equal(fx.node[1]->records, 0);
	assert_refs(1, 0, 0);
}

/* ------------------------------------------------------------------------- */
/* activation order */

/* show 3, active 1: MAIN_VIEW once (raises show, then active), then
 * AUX_VIEW twice (show only). */
static void add_activates_main_then_aux(void **state)
{
	(void)state;
	set_refs(0, 3, 1);
	assert_true(add(0, 1));
	static const long want[][2] = {{1, 0}, {1, 1}, {2, 1}, {3, 1}};
	assert_records(1, want, 4);
	assert_refs(1, 3, 1);
	assert_refs(0, 3, 1);
}

/* active above show: every activation is MAIN_VIEW. */
static void add_with_more_active_than_shown_is_all_main(void **state)
{
	(void)state;
	set_refs(0, 2, 5);
	assert_true(add(0, 1));
	static const long want[][2] = {{1, 0}, {1, 1}, {2, 1}, {2, 2}};
	assert_records(1, want, 4);
	assert_refs(1, 2, 2);
}

/* Negative counts: a negative show count activates nothing; a negative
 * active count makes every activation AUX_VIEW. */
static void add_with_negative_counts(void **state)
{
	(void)state;
	set_refs(0, -1, 3);
	assert_true(add(0, 1));
	assert_int_equal(fx.node[1]->records, 0);
	assert_refs(1, 0, 0);

	set_refs(0, 2, -1);
	assert_true(add(0, 2));
	static const long want[][2] = {{1, 0}, {2, 0}};
	assert_records(2, want, 2);
	assert_refs(2, 2, 0);
}

/* Adding twice activates twice: there is no membership check. */
static void add_twice_activates_twice(void **state)
{
	(void)state;
	set_refs(0, 1, 1);
	assert_true(add(0, 1));
	assert_true(add(0, 1));
	assert_int_equal(fx.node[1]->walks, 2);
	assert_refs(1, 2, 2);
}

/* ------------------------------------------------------------------------- */
/* remove */

/* remove mirrors add: MAIN_VIEW deactivations first (show, then active),
 * then AUX_VIEW (show only), with no walk and no cycle check. */
static void remove_deactivates_main_then_aux(void **state)
{
	(void)state;
	list(1, 0); /* a cycle add would refuse; remove does not look */
	set_refs(0, 3, 1);
	set_refs(1, 3, 1);
	capture_start();
	remove_child(0, 1);
	capture_stop();
	assert_int_equal(log_count, 0);
	static const long want[][2] = {{2, 1}, {2, 0}, {1, 0}, {0, 0}};
	assert_records(1, want, 4);
	assert_int_equal(fx.node[1]->walks, 0);
	assert_int_equal(fx.node[0]->walks, 0);
	assert_refs(1, 0, 0);
	assert_refs(0, 3, 1);
}

/* add then remove leaves the child as it was. */
static void add_then_remove_balances(void **state)
{
	(void)state;
	set_refs(0, 4, 2);
	assert_true(add(0, 1));
	assert_refs(1, 4, 2);
	remove_child(0, 1);
	assert_refs(1, 0, 0);
}

/* Each deactivation lowers only a counter above zero: a child with no
 * counts stays at zero and its tree is not walked. */
static void remove_from_inactive_child_stays_at_zero(void **state)
{
	(void)state;
	set_refs(0, 2, 2);
	remove_child(0, 1);
	assert_int_equal(fx.node[1]->records, 0);
	assert_refs(1, 0, 0);

	/* show 1, active 0, parent show 2 active 2: the first MAIN_VIEW lowers
	 * show only; nothing else is above zero. */
	set_refs(1, 1, 0);
	remove_child(0, 1);
	static const long want[][2] = {{0, 0}};
	assert_records(1, want, 1);
	assert_refs(1, 0, 0);
}

/* A hidden parent: remove deactivates nothing. */
static void remove_with_hidden_parent_does_nothing(void **state)
{
	(void)state;
	set_refs(0, 0, 3);
	set_refs(1, 2, 2);
	remove_child(0, 1);
	assert_int_equal(fx.node[1]->records, 0);
	assert_refs(1, 2, 2);
}

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	register_node();
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

#define CASE(f) cmocka_unit_test_setup_teardown(f, fixture_setup, fixture_teardown)

int main(void)
{
	const struct CMUnitTest tests[] = {
		CASE(null_parent_or_child_logs_once),
		CASE(add_parent_equals_child_warns),
		CASE(remove_parent_equals_child_deactivates_itself),
		CASE(parent_listed_by_child_is_refused),
		CASE(parent_deep_in_child_tree_is_refused),
		CASE(parent_reported_as_edge_parent_is_refused),
		CASE(real_cycle_is_refused_without_walking_parent),
		CASE(unrelated_tree_is_accepted),
		CASE(hidden_parent_accepts_without_activating),
		CASE(add_activates_main_then_aux),
		CASE(add_with_more_active_than_shown_is_all_main),
		CASE(add_with_negative_counts),
		CASE(add_twice_activates_twice),
		CASE(remove_deactivates_main_then_aux),
		CASE(add_then_remove_balances),
		CASE(remove_from_inactive_child_stays_at_zero),
		CASE(remove_with_hidden_parent_does_nothing),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

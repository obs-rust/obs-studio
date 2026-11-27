/* Characterization of obs_source_enum_full_tree() and the stop source of its
 * hidden wrapper obs_source_active_child_enum_full_tree() (#5833): the scene
 * item and transition recursion checks and obs_source_add_active_child()
 * walk every child of a source, active or not, through them.
 *
 * What it pins, on private sources of a started core, calling the exported
 * functions of the real libobs:
 *
 *   - NULL source: one LOG_DEBUG line, "obs_source_enum_full_tree: Null
 *     'source' parameter", and the callback never runs.
 *   - A root without context data (its create failed), or whose type has
 *     neither enum_all_sources nor enum_active_sources: nothing is called
 *     and nothing is logged.
 *   - A node descends through enum_all_sources when its type has it, and
 *     through enum_active_sources only when it has no enum_all_sources, at
 *     the root and for every child.
 *   - The walk is depth first and reports each edge after the child's own
 *     subtree (post-order), as (the parent the plugin reported, child,
 *     param); a shared child is walked and reported once per edge.
 *   - A child without context data is reported but not descended into.
 *   - The stop source (obs_source_add_active_child()'s parent) is reported
 *     where it turns up and never descended into; the public walk, with no
 *     stop source, descends into the same node.
 *
 * The transition slots the walk also visits need core video for a
 * transition to exist; test_source_transition_switch, test_transition_swap
 * and test_transition_swap_cycle cover them through the recursion checks,
 * and Tier 1 and Tier 3 cover them in obs-source.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * when it is installed. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#define NODE_ID "test_source_enum_full_tree_node"
#define ACTIVE_ID "test_source_enum_full_tree_active"
#define LEAF_ID "test_source_enum_full_tree_leaf"
#define NODATA_ID "test_source_enum_full_tree_nodata"
#define MAX_CHILDREN 4
#define MAX_EDGES 32

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
/* source types */

/* A node lists `all` through enum_all_sources and `active` through
 * enum_active_sources (each only where its type has that callback),
 * reporting `report_as` as their parent (itself by default), and counts the
 * calls of each. */
struct node {
	obs_source_t *self;
	obs_source_t *report_as;
	obs_source_t *all[MAX_CHILDREN];
	size_t all_count;
	obs_source_t *active[MAX_CHILDREN];
	size_t active_count;
	unsigned all_calls;
	unsigned active_calls;
};

/* The callbacks of the type whose create fails: never called with data. */
static unsigned nodata_calls;

static const char *node_name(void *type_data)
{
	(void)type_data;
	return "enum full tree node";
}

static void *node_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	struct node *node = bzalloc(sizeof(*node));
	node->self = source;
	node->report_as = source;
	return node;
}

static void *nodata_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return NULL;
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
	if (!node) {
		nodata_calls++;
		return;
	}
	node->all_calls++;
	for (size_t i = 0; i < node->all_count; i++)
		enum_callback(node->report_as, node->all[i], param);
}

static void node_enum_active(void *data, obs_source_enum_proc_t enum_callback, void *param)
{
	struct node *node = data;
	if (!node) {
		nodata_calls++;
		return;
	}
	node->active_calls++;
	for (size_t i = 0; i < node->active_count; i++)
		enum_callback(node->report_as, node->active[i], param);
}

static void register_types(void)
{
	struct obs_source_info node = {
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
	obs_register_source(&node);

	struct obs_source_info active = node;
	active.id = ACTIVE_ID;
	active.enum_all_sources = NULL;
	obs_register_source(&active);

	struct obs_source_info leaf = node;
	leaf.id = LEAF_ID;
	leaf.enum_all_sources = NULL;
	leaf.enum_active_sources = NULL;
	obs_register_source(&leaf);

	struct obs_source_info nodata = node;
	nodata.id = NODATA_ID;
	nodata.create = nodata_create;
	obs_register_source(&nodata);
}

/* ------------------------------------------------------------------------- */
/* fixture: nodes 0..3 (both callbacks), 4 active only, 5 leaf, 6 no data */

#define ACT 4
#define LEAF 5
#define NODATA 6
#define ALL 7

static obs_source_t *src[ALL];
static struct node *nodes[ALL];

static int fixture_setup(void **state)
{
	(void)state;
	static const char *names[ALL] = {"a", "b", "c", "d", "active", "leaf", "nodata"};
	for (size_t i = 0; i < ALL; i++) {
		const char *id = i == ACT ? ACTIVE_ID : i == LEAF ? LEAF_ID : i == NODATA ? NODATA_ID : NODE_ID;
		/* The failed create logs an error line: keep it out of the run's
		 * output. */
		capture_start();
		src[i] = obs_source_create_private(id, names[i], NULL);
		capture_stop();
		assert_non_null(src[i]);
		nodes[i] = obs_obj_get_data(src[i]);
		if (i == NODATA)
			assert_null(nodes[i]);
		else
			assert_non_null(nodes[i]);
	}
	nodata_calls = 0;
	return 0;
}

static int fixture_teardown(void **state)
{
	(void)state;
	for (size_t i = 0; i < ALL; i++)
		obs_source_release(src[i]);
	/* Sources are destroyed on the destruction thread, which logs. Let it
	 * finish here, so those lines never land in the next test's capture. */
	obs_wait_for_destroy_queue();
	memset(src, 0, sizeof(src));
	memset(nodes, 0, sizeof(nodes));
	return 0;
}

static void list_all(size_t parent, size_t child)
{
	struct node *node = nodes[parent];
	assert_true(node->all_count < MAX_CHILDREN);
	node->all[node->all_count++] = src[child];
}

static void list_active(size_t parent, size_t child)
{
	struct node *node = nodes[parent];
	assert_true(node->active_count < MAX_CHILDREN);
	node->active[node->active_count++] = src[child];
}

/* ------------------------------------------------------------------------- */
/* recorded edges */

struct edges {
	size_t count;
	obs_source_t *parent[MAX_EDGES];
	obs_source_t *child[MAX_EDGES];
};

static void record(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct edges *edges = param;
	if (edges->count < MAX_EDGES) {
		edges->parent[edges->count] = parent;
		edges->child[edges->count] = child;
	}
	edges->count++;
}

static void assert_edges(const struct edges *edges, const size_t (*want)[2], size_t count)
{
	assert_int_equal(edges->count, count);
	for (size_t i = 0; i < count; i++) {
		assert_ptr_equal(edges->parent[i], src[want[i][0]]);
		assert_ptr_equal(edges->child[i], src[want[i][1]]);
	}
}

/* ------------------------------------------------------------------------- */
/* cases */

static void null_source_logs_once(void **state)
{
	(void)state;
	struct edges edges = {0};

	capture_start();
	obs_source_enum_full_tree(NULL, record, &edges);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_enum_full_tree: Null 'source' parameter");
	assert_int_equal(edges.count, 0);
}

static void root_without_data_does_nothing(void **state)
{
	(void)state;
	struct edges edges = {0};

	capture_start();
	obs_source_enum_full_tree(src[NODATA], record, &edges);
	capture_stop();
	assert_int_equal(log_count, 0);
	assert_int_equal(nodata_calls, 0);
	assert_int_equal(edges.count, 0);
}

static void root_without_callbacks_does_nothing(void **state)
{
	(void)state;
	struct edges edges = {0};

	capture_start();
	obs_source_enum_full_tree(src[LEAF], record, &edges);
	capture_stop();
	assert_int_equal(log_count, 0);
	assert_int_equal(edges.count, 0);
}

/* a lists b through enum_all_sources and c through enum_active_sources; b
 * lists d and leaf the same way. Only the enum_all_sources lists are
 * walked. */
static void all_sources_preferred_at_root_and_children(void **state)
{
	(void)state;
	struct edges edges = {0};
	list_all(0, 1);
	list_active(0, 2);
	list_all(1, 3);
	list_active(1, LEAF);

	obs_source_enum_full_tree(src[0], record, &edges);
	static const size_t want[][2] = {{1, 3}, {0, 1}};
	assert_edges(&edges, want, 2);
	assert_int_equal(nodes[0]->all_calls, 1);
	assert_int_equal(nodes[0]->active_calls, 0);
	assert_int_equal(nodes[1]->all_calls, 1);
	assert_int_equal(nodes[1]->active_calls, 0);
	assert_int_equal(nodes[2]->all_calls, 0);
	assert_int_equal(nodes[3]->all_calls, 1);
}

/* active (no enum_all_sources) lists a; a lists active again below b. */
static void active_sources_used_without_all_sources(void **state)
{
	(void)state;
	struct edges edges = {0};
	list_active(ACT, 0);
	list_all(0, 1);
	list_active(1, LEAF);
	list_all(1, 2);

	obs_source_enum_full_tree(src[ACT], record, &edges);
	static const size_t want[][2] = {{1, 2}, {0, 1}, {ACT, 0}};
	assert_edges(&edges, want, 3);
	assert_int_equal(nodes[ACT]->active_calls, 1);
	assert_int_equal(nodes[0]->all_calls, 1);
	assert_int_equal(nodes[0]->active_calls, 0);
}

/* a -> b -> c, b -> leaf, a -> d */
static void tree_is_depth_first_post_order(void **state)
{
	(void)state;
	struct edges edges = {0};
	list_all(0, 1);
	list_all(0, 3);
	list_all(1, 2);
	list_all(1, LEAF);

	obs_source_enum_full_tree(src[0], record, &edges);
	static const size_t want[][2] = {{1, 2}, {1, LEAF}, {0, 1}, {0, 3}};
	assert_edges(&edges, want, 4);
	assert_int_equal(nodes[0]->all_calls, 1);
	assert_int_equal(nodes[1]->all_calls, 1);
	assert_int_equal(nodes[2]->all_calls, 1);
	assert_int_equal(nodes[3]->all_calls, 1);
}

/* a -> b, a -> c, b -> d, c -> d: d is walked and reported once per edge. */
static void shared_child_is_reported_per_edge(void **state)
{
	(void)state;
	struct edges edges = {0};
	list_all(0, 1);
	list_all(0, 2);
	list_all(1, 3);
	list_all(2, 3);

	obs_source_enum_full_tree(src[0], record, &edges);
	static const size_t want[][2] = {{1, 3}, {0, 1}, {2, 3}, {0, 2}};
	assert_edges(&edges, want, 4);
	assert_int_equal(nodes[3]->all_calls, 2);
}

static void child_without_data_is_reported_not_walked(void **state)
{
	(void)state;
	struct edges edges = {0};
	list_all(0, NODATA);
	list_all(0, 1);

	obs_source_enum_full_tree(src[0], record, &edges);
	static const size_t want[][2] = {{0, NODATA}, {0, 1}};
	assert_edges(&edges, want, 2);
	assert_int_equal(nodata_calls, 0);
}

/* The edge carries the parent the plugin passed, not the walked node. */
static void edge_carries_the_reported_parent(void **state)
{
	(void)state;
	struct edges edges = {0};
	list_all(0, 1);
	list_all(1, 2);
	nodes[1]->report_as = src[3];

	obs_source_enum_full_tree(src[0], record, &edges);
	static const size_t want[][2] = {{3, 2}, {0, 1}};
	assert_edges(&edges, want, 2);
}

/* b lists a, a lists c. obs_source_add_active_child(a, b) walks b's tree
 * with a as the stop source: a is reported, refusing the add, and its own
 * children are never walked. The public walk from b descends into a. */
static void stop_source_is_reported_not_walked(void **state)
{
	(void)state;
	list_all(1, 0);
	list_all(0, 2);

	capture_start();
	bool added = obs_source_add_active_child(src[0], src[1]);
	capture_stop();
	assert_false(added);
	assert_int_equal(log_count, 0);
	assert_int_equal(nodes[1]->all_calls, 1);
	assert_int_equal(nodes[0]->all_calls, 0);
	assert_int_equal(nodes[2]->all_calls, 0);

	struct edges edges = {0};
	obs_source_enum_full_tree(src[1], record, &edges);
	static const size_t want[][2] = {{0, 2}, {1, 0}};
	assert_edges(&edges, want, 2);
	assert_int_equal(nodes[0]->all_calls, 1);
}

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	register_types();
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
		CASE(null_source_logs_once),
		CASE(root_without_data_does_nothing),
		CASE(root_without_callbacks_does_nothing),
		CASE(all_sources_preferred_at_root_and_children),
		CASE(active_sources_used_without_all_sources),
		CASE(tree_is_depth_first_post_order),
		CASE(shared_child_is_reported_per_edge),
		CASE(child_without_data_is_reported_not_walked),
		CASE(edge_carries_the_reported_parent),
		CASE(stop_source_is_reported_not_walked),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

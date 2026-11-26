/* Characterization of obs_source_enum_active_sources() and
 * obs_source_enum_active_tree() (#5729): show and activate propagation,
 * scenes and the audio tick walk a source's active children through them.
 *
 * What it pins, on private sources of a started core, calling the exported
 * functions of the real libobs:
 *
 *   - NULL source: one LOG_DEBUG line, "<function>: Null 'source'
 *     parameter", and the callback never runs.
 *   - A source whose type has no enum_active_sources: nothing is called and
 *     nothing is logged.
 *   - enum_active_sources reports the source's direct active children only,
 *     in the plugin's order, as (source, child, param).
 *   - enum_active_tree walks depth first and reports each edge after the
 *     child's own subtree (post-order), as (edge parent, child, param); a
 *     child without enum_active_sources is reported but not descended into;
 *     a shared child is reported once per edge.
 *   - The plugin's enum_active_sources runs once per walked node with that
 *     node's own data.
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

#define NODE_ID "test_source_enum_active_node"
#define LEAF_ID "test_source_enum_active_leaf"
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

/* A node lists `children` through enum_active_sources and counts the calls. */
struct node {
	obs_source_t *self;
	obs_source_t *children[MAX_CHILDREN];
	size_t child_count;
	unsigned calls;
};

static const char *node_name(void *type_data)
{
	(void)type_data;
	return "enum active node";
}

static void *node_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	struct node *node = bzalloc(sizeof(*node));
	node->self = source;
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

static void node_enum_active(void *data, obs_source_enum_proc_t enum_callback, void *param)
{
	struct node *node = data;
	node->calls++;
	for (size_t i = 0; i < node->child_count; i++)
		enum_callback(node->self, node->children[i], param);
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
		.enum_active_sources = node_enum_active,
	};
	obs_register_source(&node);

	struct obs_source_info leaf = node;
	leaf.id = LEAF_ID;
	leaf.enum_active_sources = NULL;
	obs_register_source(&leaf);
}

/* ------------------------------------------------------------------------- */
/* fixture: nodes 0..3, leaf 4 */

#define NODES 4
#define LEAF 4
#define ALL 5

static obs_source_t *src[ALL];
static struct node *nodes[ALL];

static int fixture_setup(void **state)
{
	(void)state;
	static const char *names[ALL] = {"a", "b", "c", "d", "leaf"};
	for (size_t i = 0; i < ALL; i++) {
		src[i] = obs_source_create_private(i == LEAF ? LEAF_ID : NODE_ID, names[i], NULL);
		assert_non_null(src[i]);
		nodes[i] = obs_obj_get_data(src[i]);
		assert_non_null(nodes[i]);
	}
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

static void list(size_t parent, size_t child)
{
	struct node *node = nodes[parent];
	assert_true(node->child_count < MAX_CHILDREN);
	node->children[node->child_count++] = src[child];
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
	obs_source_enum_active_sources(NULL, record, &edges);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_enum_active_sources: Null 'source' parameter");

	capture_start();
	obs_source_enum_active_tree(NULL, record, &edges);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_enum_active_tree: Null 'source' parameter");

	assert_int_equal(edges.count, 0);
}

static void source_without_callback_does_nothing(void **state)
{
	(void)state;
	struct edges edges = {0};

	capture_start();
	obs_source_enum_active_sources(src[LEAF], record, &edges);
	obs_source_enum_active_tree(src[LEAF], record, &edges);
	capture_stop();
	assert_int_equal(log_count, 0);
	assert_int_equal(edges.count, 0);
}

static void node_without_children_calls_plugin_once(void **state)
{
	(void)state;
	struct edges edges = {0};

	obs_source_enum_active_sources(src[0], record, &edges);
	assert_int_equal(nodes[0]->calls, 1);
	obs_source_enum_active_tree(src[0], record, &edges);
	assert_int_equal(nodes[0]->calls, 2);
	assert_int_equal(edges.count, 0);
}

/* a -> b -> c, a -> leaf, b -> d */
static void build_tree(void)
{
	list(0, 1);
	list(0, LEAF);
	list(1, 2);
	list(1, 3);
}

static void sources_reports_direct_children_only(void **state)
{
	(void)state;
	struct edges edges = {0};
	build_tree();

	obs_source_enum_active_sources(src[0], record, &edges);
	static const size_t want[][2] = {{0, 1}, {0, LEAF}};
	assert_edges(&edges, want, 2);
	assert_int_equal(nodes[0]->calls, 1);
	assert_int_equal(nodes[1]->calls, 0);
}

static void tree_is_depth_first_post_order(void **state)
{
	(void)state;
	struct edges edges = {0};
	build_tree();

	obs_source_enum_active_tree(src[0], record, &edges);
	static const size_t want[][2] = {{1, 2}, {1, 3}, {0, 1}, {0, LEAF}};
	assert_edges(&edges, want, 4);
	assert_int_equal(nodes[0]->calls, 1);
	assert_int_equal(nodes[1]->calls, 1);
	assert_int_equal(nodes[2]->calls, 1);
	assert_int_equal(nodes[3]->calls, 1);
}

/* a -> b, a -> c, b -> d, c -> d: d is walked and reported once per edge. */
static void shared_child_is_reported_per_edge(void **state)
{
	(void)state;
	struct edges edges = {0};
	list(0, 1);
	list(0, 2);
	list(1, 3);
	list(2, 3);

	obs_source_enum_active_tree(src[0], record, &edges);
	static const size_t want[][2] = {{1, 3}, {0, 1}, {2, 3}, {0, 2}};
	assert_edges(&edges, want, 4);
	assert_int_equal(nodes[3]->calls, 2);
}

static void tree_from_inner_node(void **state)
{
	(void)state;
	struct edges edges = {0};
	build_tree();

	obs_source_enum_active_tree(src[1], record, &edges);
	static const size_t want[][2] = {{1, 2}, {1, 3}};
	assert_edges(&edges, want, 2);
	assert_int_equal(nodes[0]->calls, 0);
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
		CASE(source_without_callback_does_nothing),
		CASE(node_without_children_calls_plugin_once),
		CASE(sources_reports_direct_children_only),
		CASE(tree_is_depth_first_post_order),
		CASE(shared_child_is_reported_per_edge),
		CASE(tree_from_inner_node),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

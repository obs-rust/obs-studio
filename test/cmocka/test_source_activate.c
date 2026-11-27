/* Characterization of obs_source_activate() and obs_source_deactivate()
 * (#5811), the internal calls behind obs_source_inc_showing(),
 * obs_source_inc_active(), obs_source_dec_showing() and
 * obs_source_dec_active(), views, canvases and active children.
 *
 * Both are declared in obs-internal.h without EXPORT, so the test reaches
 * them through the four public counters: inc_showing activates for
 * AUX_VIEW, inc_active for MAIN_VIEW, and the dec pair deactivates the
 * same way. (Their own NULL check can't be reached from outside libobs:
 * every caller checks first. Tier 1 and Tier 3 cover it.)
 *
 * What it pins, on private sources of a started core whose
 * enum_active_sources lists children (a lists b and d, b lists c), with
 * every visit of the walk recording all eight counters at that moment:
 *
 *   - AUX activate: the root's show_refs +1, then one walk of the active
 *     tree (children before their parent's own step, post-order) that adds
 *     1 to each descendant's show_refs. activate_refs never moves.
 *   - MAIN activate: the same, then the root's activate_refs +1 and a second
 *     walk that adds 1 to each descendant's activate_refs.
 *   - AUX deactivate: only when the root's show_refs is above zero, -1 and a
 *     walk subtracting 1 from each descendant's show_refs. A descendant's
 *     count is not floor-checked: it goes below zero.
 *   - MAIN deactivate: that, then, only when the root's activate_refs is
 *     above zero, the same for activate_refs. Each counter is checked on
 *     its own: a hidden root that is still active walks once.
 *   - A root at zero is left alone and is never walked.
 *   - Nothing is logged.
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

#define NODE_ID "test_source_activate_node"
#define NODES 4
#define MAX_CHILDREN 2
#define MAX_VISITS 32

/* ------------------------------------------------------------------------- */
/* log capture */

static unsigned log_count;
#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
	log_count++;
}

static log_handler_t saved_handler;
static void *saved_param;

static void capture_start(void)
{
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
}

static void capture_stop(void)
{
	base_set_log_handler(saved_handler, saved_param);
}

/* ------------------------------------------------------------------------- */
/* the node source type */

/* One visit of the walk: which node's enum_active_sources ran, and every
 * node's show_refs and activate_refs at that moment. */
struct visit {
	size_t node;
	long show[NODES];
	long active[NODES];
};

struct node {
	size_t index;
	obs_source_t *children[MAX_CHILDREN];
	size_t child_count;
};

static obs_source_t *sources[NODES];
static struct visit visits[MAX_VISITS];
static size_t visit_count;

static const char *node_name(void *type_data)
{
	(void)type_data;
	return "activate node";
}

static void *node_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(sizeof(struct node));
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
	if (visit_count < MAX_VISITS) {
		struct visit *v = &visits[visit_count];
		v->node = node->index;
		for (size_t i = 0; i < NODES; i++) {
			v->show[i] = os_atomic_load_long(&sources[i]->show_refs);
			v->active[i] = os_atomic_load_long(&sources[i]->activate_refs);
		}
	}
	visit_count++;
	for (size_t i = 0; i < node->child_count; i++)
		enum_callback(sources[node->index], node->children[i], param);
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
		.enum_active_sources = node_enum_active,
	};
	obs_register_source(&info);
}

/* ------------------------------------------------------------------------- */
/* fixture: a lists b and d, b lists c */

enum { A, B, C, D };

static int fixture_setup(void **state)
{
	(void)state;
	static const char *names[NODES] = {"a", "b", "c", "d"};
	struct node *nodes[NODES];
	for (size_t i = 0; i < NODES; i++) {
		sources[i] = obs_source_create_private(NODE_ID, names[i], NULL);
		assert_non_null(sources[i]);
		nodes[i] = obs_obj_get_data(sources[i]);
		assert_non_null(nodes[i]);
		nodes[i]->index = i;
	}
	nodes[A]->children[0] = sources[B];
	nodes[A]->children[1] = sources[D];
	nodes[A]->child_count = 2;
	nodes[B]->children[0] = sources[C];
	nodes[B]->child_count = 1;
	visit_count = 0;
	memset(visits, 0, sizeof(visits));
	return 0;
}

static int fixture_teardown(void **state)
{
	(void)state;
	for (size_t i = 0; i < NODES; i++) {
		os_atomic_set_long(&sources[i]->show_refs, 0);
		os_atomic_set_long(&sources[i]->activate_refs, 0);
		obs_source_release(sources[i]);
		sources[i] = NULL;
	}
	/* Sources are destroyed on the destruction thread, which logs. Let it
	 * finish here, so those lines never land in the next test's capture. */
	obs_wait_for_destroy_queue();
	return 0;
}

/* ------------------------------------------------------------------------- */
/* helpers */

/* Counters as {a, b, c, d}. */
struct counters {
	long show[NODES];
	long active[NODES];
};

static void set_counters(const struct counters *c)
{
	for (size_t i = 0; i < NODES; i++) {
		os_atomic_set_long(&sources[i]->show_refs, c->show[i]);
		os_atomic_set_long(&sources[i]->activate_refs, c->active[i]);
	}
}

static void assert_counters(const struct counters *c)
{
	for (size_t i = 0; i < NODES; i++) {
		assert_int_equal(os_atomic_load_long(&sources[i]->show_refs), c->show[i]);
		assert_int_equal(os_atomic_load_long(&sources[i]->activate_refs), c->active[i]);
	}
}

static void assert_visit(size_t index, size_t node, const struct counters *c)
{
	assert_true(index < visit_count);
	assert_int_equal(visits[index].node, node);
	for (size_t i = 0; i < NODES; i++) {
		assert_int_equal(visits[index].show[i], c->show[i]);
		assert_int_equal(visits[index].active[i], c->active[i]);
	}
}

static void run(void (*call)(obs_source_t *))
{
	visit_count = 0;
	capture_start();
	call(sources[A]);
	capture_stop();
	assert_int_equal(log_count, 0);
}

/* ------------------------------------------------------------------------- */
/* activate */

/* One walk, the order of its visits: a's own enum first, then b (whose child
 * c is raised before b), then d. Each descendant is raised after its own
 * enum ran, so a visit sees its node at the old value. */
static void aux_activate_raises_show_refs_in_one_walk(void **state)
{
	(void)state;
	run(obs_source_inc_showing);
	assert_int_equal(visit_count, 4);
	/* a's show_refs rose before the walk started. */
	assert_visit(0, A, &(struct counters){{1, 0, 0, 0}, {0, 0, 0, 0}});
	assert_visit(1, B, &(struct counters){{1, 0, 0, 0}, {0, 0, 0, 0}});
	assert_visit(2, C, &(struct counters){{1, 0, 0, 0}, {0, 0, 0, 0}});
	/* c, then b, were raised before d was visited. */
	assert_visit(3, D, &(struct counters){{1, 1, 1, 0}, {0, 0, 0, 0}});
	assert_counters(&(struct counters){{1, 1, 1, 1}, {0, 0, 0, 0}});
}

/* The show walk, then a's activate_refs and a second walk. */
static void main_activate_walks_show_then_active(void **state)
{
	(void)state;
	run(obs_source_inc_active);
	assert_int_equal(visit_count, 8);
	assert_visit(0, A, &(struct counters){{1, 0, 0, 0}, {0, 0, 0, 0}});
	assert_visit(3, D, &(struct counters){{1, 1, 1, 0}, {0, 0, 0, 0}});
	/* The second walk starts after every show_refs rose and a's
	 * activate_refs with them. */
	assert_visit(4, A, &(struct counters){{1, 1, 1, 1}, {1, 0, 0, 0}});
	assert_visit(5, B, &(struct counters){{1, 1, 1, 1}, {1, 0, 0, 0}});
	assert_visit(6, C, &(struct counters){{1, 1, 1, 1}, {1, 0, 0, 0}});
	assert_visit(7, D, &(struct counters){{1, 1, 1, 1}, {1, 1, 1, 0}});
	assert_counters(&(struct counters){{1, 1, 1, 1}, {1, 1, 1, 1}});
}

/* Activation never looks at the current values: from any count, +1. */
static void activate_adds_one_to_any_count(void **state)
{
	(void)state;
	set_counters(&(struct counters){{-3, 0, 7, 2}, {5, -1, 0, 9}});
	run(obs_source_inc_active);
	assert_int_equal(visit_count, 8);
	assert_counters(&(struct counters){{-2, 1, 8, 3}, {6, 0, 1, 10}});
}

/* ------------------------------------------------------------------------- */
/* deactivate */

static void aux_deactivate_lowers_show_refs_in_one_walk(void **state)
{
	(void)state;
	set_counters(&(struct counters){{2, 2, 2, 2}, {1, 1, 1, 1}});
	run(obs_source_dec_showing);
	assert_int_equal(visit_count, 4);
	assert_visit(0, A, &(struct counters){{1, 2, 2, 2}, {1, 1, 1, 1}});
	assert_visit(3, D, &(struct counters){{1, 1, 1, 2}, {1, 1, 1, 1}});
	assert_counters(&(struct counters){{1, 1, 1, 1}, {1, 1, 1, 1}});
}

static void main_deactivate_walks_show_then_active(void **state)
{
	(void)state;
	set_counters(&(struct counters){{1, 1, 1, 1}, {1, 1, 1, 1}});
	run(obs_source_dec_active);
	assert_int_equal(visit_count, 8);
	assert_visit(0, A, &(struct counters){{0, 1, 1, 1}, {1, 1, 1, 1}});
	assert_visit(3, D, &(struct counters){{0, 0, 0, 1}, {1, 1, 1, 1}});
	assert_visit(4, A, &(struct counters){{0, 0, 0, 0}, {0, 1, 1, 1}});
	assert_visit(7, D, &(struct counters){{0, 0, 0, 0}, {0, 0, 0, 1}});
	assert_counters(&(struct counters){{0, 0, 0, 0}, {0, 0, 0, 0}});
}

/* Only the root is checked: descendants go below zero. */
static void descendants_are_not_floor_checked(void **state)
{
	(void)state;
	set_counters(&(struct counters){{1, 0, -4, 0}, {1, 0, 0, 3}});
	run(obs_source_dec_active);
	assert_int_equal(visit_count, 8);
	assert_counters(&(struct counters){{0, -1, -5, -1}, {0, -1, -1, 2}});
}

/* A root at zero is left alone and never walked. */
static void deactivate_at_zero_does_nothing(void **state)
{
	(void)state;
	set_counters(&(struct counters){{0, 3, 3, 3}, {0, 3, 3, 3}});
	run(obs_source_dec_showing);
	assert_int_equal(visit_count, 0);
	run(obs_source_dec_active);
	assert_int_equal(visit_count, 0);
	set_counters(&(struct counters){{-2, 3, 3, 3}, {-1, 3, 3, 3}});
	run(obs_source_dec_active);
	assert_int_equal(visit_count, 0);
	assert_counters(&(struct counters){{-2, 3, 3, 3}, {-1, 3, 3, 3}});
}

/* Each counter is checked on its own: a hidden root that is still active
 * skips the show walk and takes the active one. */
static void hidden_but_active_root_walks_once(void **state)
{
	(void)state;
	set_counters(&(struct counters){{0, 2, 2, 2}, {1, 2, 2, 2}});
	run(obs_source_dec_active);
	assert_int_equal(visit_count, 4);
	assert_visit(0, A, &(struct counters){{0, 2, 2, 2}, {0, 2, 2, 2}});
	assert_counters(&(struct counters){{0, 2, 2, 2}, {0, 1, 1, 1}});
}

/* dec_showing leaves activate_refs alone even when only it is set. */
static void aux_deactivate_ignores_activate_refs(void **state)
{
	(void)state;
	set_counters(&(struct counters){{0, 2, 2, 2}, {4, 2, 2, 2}});
	run(obs_source_dec_showing);
	assert_int_equal(visit_count, 0);
	assert_counters(&(struct counters){{0, 2, 2, 2}, {4, 2, 2, 2}});
}

/* A balanced sequence ends where it started, on every node. */
static void balanced_sequence_ends_at_zero(void **state)
{
	(void)state;
	run(obs_source_inc_active);
	run(obs_source_inc_showing);
	run(obs_source_inc_active);
	assert_counters(&(struct counters){{3, 3, 3, 3}, {2, 2, 2, 2}});
	run(obs_source_dec_showing);
	run(obs_source_dec_active);
	run(obs_source_dec_active);
	assert_counters(&(struct counters){{0, 0, 0, 0}, {0, 0, 0, 0}});
	run(obs_source_dec_active);
	assert_int_equal(visit_count, 0);
	assert_counters(&(struct counters){{0, 0, 0, 0}, {0, 0, 0, 0}});
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
		CASE(aux_activate_raises_show_refs_in_one_walk),
		CASE(main_activate_walks_show_then_active),
		CASE(activate_adds_one_to_any_count),
		CASE(aux_deactivate_lowers_show_refs_in_one_walk),
		CASE(main_deactivate_walks_show_then_active),
		CASE(descendants_are_not_floor_checked),
		CASE(deactivate_at_zero_does_nothing),
		CASE(hidden_but_active_root_walks_once),
		CASE(aux_deactivate_ignores_activate_refs),
		CASE(balanced_sequence_ends_at_zero),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}

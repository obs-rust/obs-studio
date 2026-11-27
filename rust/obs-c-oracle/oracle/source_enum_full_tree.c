/* Tier 3: the unchanged extracted obs_source_enum_full_tree() and
 * obs_source_active_child_enum_full_tree(), renamed oracle_*, and the
 * production field adapters the Rust shims go through
 * (obs-source-enum-full-tree-fields.c.h, defined once, here), over the real
 * private layout.
 *
 * Both sides reach the same recording stand-ins for obs_source_get_ref(),
 * obs_source_release() and obs_transition_enum_sources() (the original C
 * through macros, the Rust shims through their Calls), and the same scripted
 * enum_all_sources and enum_active_sources callbacks, which report a case's
 * children in order. The real calls are covered by
 * test_source_enum_full_tree. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_enum_full_tree oracle_obs_source_enum_full_tree
#define obs_source_active_child_enum_full_tree oracle_obs_source_active_child_enum_full_tree
#define obs_source_get_ref oracle_full_tree_get_ref
#define obs_source_release oracle_full_tree_release
#define obs_transition_enum_sources oracle_full_tree_transition_enum
#include "libobs/build/libobs.pch.h"
#include "source-enum-full-tree.inc"
#undef obs_transition_enum_sources
#undef obs_source_release
#undef obs_source_get_ref
#undef obs_source_active_child_enum_full_tree
#undef obs_source_enum_full_tree
#include "source_enum_full_tree_fields.inc"

#define FULL_TREE_NODES 5
#define FULL_TREE_CHILDREN 3
#define FULL_TREE_EVENTS 96
/* A child entry for NULL; the index recorded for NULL and for a pointer that
 * is no node. */
#define FULL_TREE_CHILD_NULL (-2)
#define FULL_TREE_NULL 0xfe
#define FULL_TREE_OTHER 0xff

/* Node flags. */
#define FULL_TREE_TRANSITION 1
#define FULL_TREE_ALL 2
#define FULL_TREE_ACTIVE 4
#define FULL_TREE_DATA 8
#define FULL_TREE_DYING 16

/* One call: obs_source_enum_full_tree (which 0) or the hidden walk with stop
 * source `stop` (which 1; -1 for NULL) on node `root` (-1 for NULL). Each
 * node has flags, up to three children for each callback and two transition
 * slots (-1 for none). A child or slot at or below its node's index is
 * skipped, so every walk ends; a NULL child (FULL_TREE_CHILD_NULL) is
 * reported only when the stop source is NULL (the C dereferences it
 * otherwise). */
struct full_tree_case {
	uint8_t which;
	int8_t root;
	int8_t stop;
	uint8_t noise;
	uint8_t flags[FULL_TREE_NODES];
	int8_t all[FULL_TREE_NODES][FULL_TREE_CHILDREN];
	int8_t active[FULL_TREE_NODES][FULL_TREE_CHILDREN];
	int8_t slots[FULL_TREE_NODES][2];
};

/* What one side did: its events, three words each (1 enum_all_sources and
 * 6 enum_active_sources: node, how the callback relates to the caller's;
 * 2 get_ref: node; 3 release: node; 4 transition enum: node, as 1; 5 the
 * caller's callback: parent, child), its log lines and whether every byte of
 * the nodes is as before. */
struct full_tree_result {
	uint32_t events;
	uint64_t trace[3 * FULL_TREE_EVENTS];
	uint32_t log_count;
	int32_t log_level;
	char log[160];
	bool untouched;
};

struct full_tree_fixture {
	struct obs_source nodes[FULL_TREE_NODES];
	uint8_t script[FULL_TREE_NODES];
	const struct full_tree_case *c;
	struct full_tree_result *out;
};

static _Thread_local struct full_tree_fixture *full_tree_current;
static int full_tree_user_param;

static uint64_t full_tree_index(const void *source)
{
	struct full_tree_fixture *f = full_tree_current;
	if (!source)
		return FULL_TREE_NULL;
	for (size_t i = 0; f && i < FULL_TREE_NODES; i++) {
		if (source == &f->nodes[i])
			return i;
	}
	return FULL_TREE_OTHER;
}

static void full_tree_record(uint64_t kind, uint64_t a, uint64_t b)
{
	struct full_tree_result *out = full_tree_current->out;
	if (out->events < FULL_TREE_EVENTS) {
		out->trace[3 * out->events] = kind;
		out->trace[3 * out->events + 1] = a;
		out->trace[3 * out->events + 2] = b;
	}
	out->events++;
}

static void full_tree_user(obs_source_t *parent, obs_source_t *child, void *param)
{
	full_tree_record(5, full_tree_index(parent),
			 full_tree_index(child) | (param == &full_tree_user_param ? 0 : 0x100));
}

static uint64_t full_tree_is_user(obs_source_enum_proc_t cb, void *param)
{
	return (cb == full_tree_user ? 1 : 0) | (param == &full_tree_user_param ? 2 : 0);
}

static void full_tree_report(size_t i, const int8_t *list, obs_source_enum_proc_t cb, void *param)
{
	struct full_tree_fixture *f = full_tree_current;
	for (size_t k = 0; k < FULL_TREE_CHILDREN; k++) {
		int8_t child = list[k];
		if (child == FULL_TREE_CHILD_NULL && f->c->stop < 0)
			cb(&f->nodes[i], NULL, param);
		else if (child > (int8_t)i && child < FULL_TREE_NODES)
			cb(&f->nodes[i], &f->nodes[child], param);
	}
}

static void full_tree_plugin_all(void *data, obs_source_enum_proc_t cb, void *param)
{
	struct full_tree_fixture *f = full_tree_current;
	size_t i = (size_t)((uint8_t *)data - f->script);
	full_tree_record(1, i, full_tree_is_user(cb, param));
	full_tree_report(i, f->c->all[i], cb, param);
}

static void full_tree_plugin_active(void *data, obs_source_enum_proc_t cb, void *param)
{
	struct full_tree_fixture *f = full_tree_current;
	size_t i = (size_t)((uint8_t *)data - f->script);
	full_tree_record(6, i, full_tree_is_user(cb, param));
	full_tree_report(i, f->c->active[i], cb, param);
}

obs_source_t *oracle_full_tree_get_ref(obs_source_t *source);
obs_source_t *oracle_full_tree_get_ref(obs_source_t *source)
{
	uint64_t i = full_tree_index(source);
	full_tree_record(2, i, 0);
	if (i < FULL_TREE_NODES && (full_tree_current->c->flags[i] & FULL_TREE_DYING))
		return NULL;
	return source;
}

void oracle_full_tree_release(obs_source_t *source);
void oracle_full_tree_release(obs_source_t *source)
{
	full_tree_record(3, full_tree_index(source), 0);
}

void oracle_full_tree_transition_enum(obs_source_t *source, obs_source_enum_proc_t cb, void *param);
void oracle_full_tree_transition_enum(obs_source_t *source, obs_source_enum_proc_t cb, void *param)
{
	struct full_tree_fixture *f = full_tree_current;
	uint64_t i = full_tree_index(source);
	full_tree_record(4, i, full_tree_is_user(cb, param));
	if (i >= FULL_TREE_NODES)
		return;
	for (size_t k = 0; k < 2; k++) {
		int8_t slot = f->c->slots[i][k];
		if (slot > (int8_t)i && slot < FULL_TREE_NODES)
			cb(source, &f->nodes[slot], param);
	}
}

#if defined(__GNUC__) || defined(__clang__)
static void full_tree_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void full_tree_capture(int level, const char *format, va_list args, void *param)
{
	struct full_tree_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

typedef void (*full_tree_fn)(obs_source_t *, obs_source_enum_proc_t, void *);
typedef void (*full_tree_stop_fn)(obs_source_t *, obs_source_enum_proc_t, void *, obs_source_t *);
typedef void (*full_tree_get_log)(log_handler_t *, void **);
typedef void (*full_tree_set_log)(log_handler_t, void *);

/* The original calls, for the parity test's other side. */
void oracle_source_enum_full_tree_originals(full_tree_fn *walk, full_tree_stop_fn *stop_walk);
void oracle_source_enum_full_tree_originals(full_tree_fn *walk, full_tree_stop_fn *stop_walk)
{
	*walk = oracle_obs_source_enum_full_tree;
	*stop_walk = oracle_obs_source_active_child_enum_full_tree;
}

/* Runs one case through either call on fresh noise-filled nodes. False when
 * the fixture could not be built or an adapter does not reach its field. */
bool oracle_source_enum_full_tree_run(full_tree_fn walk, full_tree_stop_fn stop_walk, full_tree_get_log get,
				      full_tree_set_log set, const struct full_tree_case *c,
				      struct full_tree_result *out);
bool oracle_source_enum_full_tree_run(full_tree_fn walk, full_tree_stop_fn stop_walk, full_tree_get_log get,
				      full_tree_set_log set, const struct full_tree_case *c,
				      struct full_tree_result *out)
{
	struct full_tree_fixture *f = malloc(sizeof(*f));
	struct obs_source *before = malloc(sizeof(f->nodes));
	if (!f || !before) {
		free(f);
		free(before);
		return false;
	}
	memset(f, c->noise, sizeof(*f));
	memset(out, 0, sizeof(*out));
	out->log_level = -1;
	f->c = c;
	f->out = out;
	bool valid = true;
	for (size_t i = 0; i < FULL_TREE_NODES; i++) {
		uint8_t flags = c->flags[i];
		f->nodes[i].context.data = (flags & FULL_TREE_DATA) ? &f->script[i] : NULL;
		f->nodes[i].info.type = (flags & FULL_TREE_TRANSITION)
						? OBS_SOURCE_TYPE_TRANSITION
						: (enum obs_source_type)(c->noise % 3 == 0 ? OBS_SOURCE_TYPE_INPUT
											   : OBS_SOURCE_TYPE_FILTER);
		f->nodes[i].info.enum_all_sources = (flags & FULL_TREE_ALL) ? full_tree_plugin_all : NULL;
		f->nodes[i].info.enum_active_sources = (flags & FULL_TREE_ACTIVE) ? full_tree_plugin_active : NULL;
		valid = valid && obs_source_enum_full_tree_has_data(&f->nodes[i]) == ((flags & FULL_TREE_DATA) != 0) &&
			obs_source_enum_full_tree_is_transition(&f->nodes[i]) ==
				((flags & FULL_TREE_TRANSITION) != 0) &&
			obs_source_enum_full_tree_has_enum_all(&f->nodes[i]) == ((flags & FULL_TREE_ALL) != 0) &&
			obs_source_enum_full_tree_has_enum_active(&f->nodes[i]) == ((flags & FULL_TREE_ACTIVE) != 0);
	}
	memcpy(before, f->nodes, sizeof(f->nodes));

	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	full_tree_current = f;
	set(full_tree_capture, out);
	obs_source_t *root = c->root < 0 || c->root >= FULL_TREE_NODES ? NULL : &f->nodes[c->root];
	obs_source_t *stop = c->stop < 0 || c->stop >= FULL_TREE_NODES ? NULL : &f->nodes[c->stop];
	if (c->which == 0)
		walk(root, full_tree_user, &full_tree_user_param);
	else
		stop_walk(root, full_tree_user, &full_tree_user_param, stop);
	set(old_handler, old_param);
	full_tree_current = NULL;

	out->untouched = memcmp(before, f->nodes, sizeof(f->nodes)) == 0;
	free(before);
	free(f);
	return valid;
}

/* Tier 3: the unchanged extracted obs_source_add_active_child() and
 * obs_source_remove_active_child(), renamed oracle_*, and the production
 * counter adapters the Rust shims go through
 * (obs-source-active-child-fields.c.h, defined once, here), over the real
 * private layout.
 *
 * Both sides reach the same stand-ins: the full-tree walk (the hidden
 * obs_source_active_child_enum_full_tree(), Rust since #5833, replaced here
 * by oracle_active_child_enum_full_tree(), a scripted walk that reports a
 * case's edges in order; the original C reaches it through a macro, the
 * Rust shims through the parity test's Libobs) and obs_source_activate() /
 * obs_source_deactivate(),
 * obs-source's Rust exports (#5811), whose counter adapters report to
 * oracle/view.c's activation trace for these nodes (they leave the
 * counters alone), which reads a source's name at offset 0, where struct
 * obs_source keeps context.name. The real walk and activation are covered
 * by test_source_active_child and test_source_activate. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_add_active_child oracle_obs_source_add_active_child
#define obs_source_remove_active_child oracle_obs_source_remove_active_child
#define obs_source_active_child_enum_full_tree oracle_active_child_enum_full_tree
#include "libobs/build/libobs.pch.h"
#include "source-active-child.inc"
#undef obs_source_active_child_enum_full_tree
#undef obs_source_remove_active_child
#undef obs_source_add_active_child
#include "source_active_child_fields.inc"

#define ACTIVE_CHILD_NODES 4
#define ACTIVE_CHILD_EDGES 8
#define ACTIVE_CHILD_EVENTS 24
/* An index for NULL, and for a pointer that is no node. */
#define ACTIVE_CHILD_NULL 0xfe
#define ACTIVE_CHILD_OTHER 0xff

/* One call: add (which 0) or remove (1) of node `child` under node `parent`
 * (-1 for NULL). The parent's counters are show_refs and activate_refs;
 * every other node's are noise. The walk reports `edges` edges, each
 * between two nodes or NULL (-1). With `clobber`, each activation or
 * deactivation zeroes the parent's counters, so a call that loads them
 * again after the first one shows it. */
struct active_child_case {
	uint8_t which;
	int8_t parent;
	int8_t child;
	uint8_t noise;
	long show_refs;
	long activate_refs;
	uint8_t edges;
	int8_t edge_parent[ACTIVE_CHILD_EDGES];
	int8_t edge_child[ACTIVE_CHILD_EDGES];
	bool clobber;
};

/* What one side did: its events in order, three words each (1 walk: root,
 * stop; 2 activate and 3 deactivate: source, view type), what it returned
 * (add only), its log lines (count, level of the last, the last), and
 * whether every byte of the nodes but the clobbered counters is as
 * before. */
struct active_child_result {
	uint32_t events;
	uint64_t trace[3 * ACTIVE_CHILD_EVENTS];
	bool returned;
	uint32_t log_count;
	int32_t log_level;
	char log[160];
	bool untouched;
};

struct active_child_fixture {
	struct obs_source nodes[ACTIVE_CHILD_NODES];
	const struct active_child_case *c;
	struct active_child_result *out;
};

static char active_child_names[ACTIVE_CHILD_NODES][4] = {"n0", "n1", "n2", "n3"};

static _Thread_local struct active_child_fixture *active_child_current;

static uint64_t active_child_index(const void *source)
{
	struct active_child_fixture *f = active_child_current;
	if (!source)
		return ACTIVE_CHILD_NULL;
	for (size_t i = 0; f && i < ACTIVE_CHILD_NODES; i++) {
		if (source == &f->nodes[i])
			return i;
	}
	return ACTIVE_CHILD_OTHER;
}

static obs_source_t *active_child_node(int8_t index)
{
	struct active_child_fixture *f = active_child_current;
	return index < 0 || index >= ACTIVE_CHILD_NODES ? NULL : &f->nodes[index];
}

static void active_child_record(uint64_t kind, uint64_t a, uint64_t b)
{
	struct active_child_result *out = active_child_current->out;
	if (out->events < ACTIVE_CHILD_EVENTS) {
		out->trace[3 * out->events] = kind;
		out->trace[3 * out->events + 1] = a;
		out->trace[3 * out->events + 2] = b;
	}
	out->events++;
}

/* The scripted walk both sides call: one event, then the case's edges in
 * order. */
void oracle_active_child_enum_full_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param,
					obs_source_t *stop_source)
{
	struct active_child_fixture *f = active_child_current;
	if (!f)
		return;
	active_child_record(1, active_child_index(source), active_child_index(stop_source));
	for (uint8_t i = 0; i < f->c->edges && i < ACTIVE_CHILD_EDGES; i++)
		enum_callback(active_child_node(f->c->edge_parent[i]), active_child_node(f->c->edge_child[i]), param);
}

typedef void (*oracle_view_trace_fn)(const char *event, const char *name, long long value);
void oracle_view_set_trace(oracle_view_trace_fn hook);
void oracle_view_flush(void);

static void active_child_trace(const char *event, const char *name, long long value)
{
	struct active_child_fixture *f = active_child_current;
	if (!f)
		return;
	uint64_t index = name ? ACTIVE_CHILD_OTHER : ACTIVE_CHILD_NULL;
	for (size_t i = 0; name && i < ACTIVE_CHILD_NODES; i++) {
		if (name == active_child_names[i])
			index = i;
	}
	uint64_t kind = strcmp(event, "activate") == 0 ? 2 : strcmp(event, "deactivate") == 0 ? 3 : 9;
	active_child_record(kind, index, (uint64_t)value);
	obs_source_t *parent = active_child_node(f->c->parent);
	if (f->c->clobber && parent) {
		os_atomic_set_long(&parent->show_refs, 0);
		os_atomic_set_long(&parent->activate_refs, 0);
	}
}

#if defined(__GNUC__) || defined(__clang__)
static void active_child_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void active_child_capture(int level, const char *format, va_list args, void *param)
{
	struct active_child_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

typedef bool (*active_child_add_fn)(obs_source_t *, obs_source_t *);
typedef void (*active_child_remove_fn)(obs_source_t *, obs_source_t *);
typedef void (*active_child_get_log)(log_handler_t *, void **);
typedef void (*active_child_set_log)(log_handler_t, void *);

/* The original calls, for the parity test's other side. */
void oracle_source_active_child_originals(active_child_add_fn *add, active_child_remove_fn *remove);
void oracle_source_active_child_originals(active_child_add_fn *add, active_child_remove_fn *remove)
{
	*add = oracle_obs_source_add_active_child;
	*remove = oracle_obs_source_remove_active_child;
}

/* Runs one case through add or remove on fresh noise-filled nodes. False
 * when the fixture could not be built or an adapter does not reach its
 * field. */
bool oracle_source_active_child_run(active_child_add_fn add, active_child_remove_fn remove, active_child_get_log get,
				    active_child_set_log set, const struct active_child_case *c,
				    struct active_child_result *out);
bool oracle_source_active_child_run(active_child_add_fn add, active_child_remove_fn remove, active_child_get_log get,
				    active_child_set_log set, const struct active_child_case *c,
				    struct active_child_result *out)
{
	struct active_child_fixture *f = malloc(sizeof(*f));
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
	for (size_t i = 0; i < ACTIVE_CHILD_NODES; i++) {
		f->nodes[i].context.name = active_child_names[i];
		/* Each node's counters differ from the parent's. */
		f->nodes[i].show_refs = (long)(c->noise % 7) + 100 * (long)i + 11;
		f->nodes[i].activate_refs = (long)(c->noise % 5) + 100 * (long)i + 13;
	}
	if (c->parent >= 0 && c->parent < ACTIVE_CHILD_NODES) {
		f->nodes[c->parent].show_refs = c->show_refs;
		f->nodes[c->parent].activate_refs = c->activate_refs;
	}
	/* The adapters reach the fields they name. */
	for (size_t i = 0; i < ACTIVE_CHILD_NODES; i++) {
		valid = valid && obs_source_active_child_show_refs(&f->nodes[i]) == f->nodes[i].show_refs &&
			obs_source_active_child_activate_refs(&f->nodes[i]) == f->nodes[i].activate_refs;
	}
	memcpy(before, f->nodes, sizeof(f->nodes));

	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	active_child_current = f;
	oracle_view_set_trace(active_child_trace);
	set(active_child_capture, out);
	obs_source_t *parent = active_child_node(c->parent);
	obs_source_t *child = active_child_node(c->child);
	if (c->which == 0)
		out->returned = add(parent, child);
	else
		remove(parent, child);
	oracle_view_flush();
	set(old_handler, old_param);
	oracle_view_set_trace(NULL);
	active_child_current = NULL;

	if (c->clobber && parent) {
		size_t p = (size_t)c->parent;
		before[p].show_refs = f->nodes[p].show_refs;
		before[p].activate_refs = f->nodes[p].activate_refs;
	}
	out->untouched = memcmp(before, f->nodes, sizeof(f->nodes)) == 0;
	free(before);
	free(f);
	return valid;
}

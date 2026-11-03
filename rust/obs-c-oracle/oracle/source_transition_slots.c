/* Test-only oracle: the original libobs/obs-source-transition-slots.c.h,
 * unmodified, with its four functions renamed to oracle_* so they can link
 * next to the Rust shims (rust/obs-source-seam/src/ffi/source_transition_slots.rs).
 *
 * Both sides reach the transition fields and locks through the same
 * transition seam, libobs/obs-source-transition-seam.c.h, which
 * oracle/source_transition_settings.c compiles under its own names, as the
 * Rust shims call it in the hybrid libobs. The checks the swap asks
 * (obs_source_transition_start_child_active, obs_source_transition_load_*)
 * are the Rust ones, as in the hybrid libobs; blog() is base-variadic.c.
 *
 * The slots' calls into the rest of libobs (obs_source_get_ref,
 * obs_source_release, obs_source_add_active_child,
 * obs_source_remove_active_child, obs_source_get_name) are renamed to the
 * recording stand-ins below, which the parity test hands the Rust shims
 * too (their Calls), so both sides run on one graph: each source has a
 * reference count, each parent and child an active-child link count, and
 * every call is traced in order.
 *
 * A test source is a zeroed struct obs_source with its type, name and both
 * transition mutexes. */

#define obs_transition_clear oracle_obs_transition_clear
#define obs_transition_enum_sources oracle_obs_transition_enum_sources
#define obs_transition_swap_begin oracle_obs_transition_swap_begin
#define obs_transition_swap_end oracle_obs_transition_swap_end
#define obs_source_get_ref oracle_slots_get_ref
#define obs_source_release oracle_slots_release
#define obs_source_add_active_child oracle_slots_add_active_child
#define obs_source_remove_active_child oracle_slots_remove_active_child
#define obs_source_get_name oracle_slots_get_name

/* the slots file's precompiled header brings obs.h, so the oracle_*
 * prototypes come from obs.h through the renames */
#include "obs-source-transition-slots.c.h"

#undef obs_source_get_name
#undef obs_source_remove_active_child
#undef obs_source_add_active_child
#undef obs_source_release
#undef obs_source_get_ref
#undef obs_transition_swap_end
#undef obs_transition_swap_begin
#undef obs_transition_enum_sources
#undef obs_transition_clear

#define ORACLE_SLOTS_SOURCES 16
#define ORACLE_SLOTS_LINKS 32
#define ORACLE_SLOTS_TRACE 128

/* A traced call: kind (1 get_ref, 2 release, 3 add, 4 remove), the sources
 * it names (NULL where it has none) and what it returned. obs_source_get_name
 * is not traced: the C evaluates the warning's two names in either order. */
struct oracle_slots_event {
	int kind;
	obs_source_t *a;
	obs_source_t *b;
	int result;
};

static struct {
	obs_source_t *source[ORACLE_SLOTS_SOURCES];
	long long refs[ORACLE_SLOTS_SOURCES];
	size_t num;
	struct {
		obs_source_t *parent;
		obs_source_t *child;
		long long count;
		bool refuses;
	} link[ORACLE_SLOTS_LINKS];
	size_t links;
	struct oracle_slots_event trace[ORACLE_SLOTS_TRACE];
	size_t traced;
} graph;

obs_source_t *oracle_slots_source_new(int type, const char *name);
void oracle_slots_reset(void);
long long oracle_slots_refs(const obs_source_t *source);
void oracle_slots_set_refs(obs_source_t *source, long long refs);
long long oracle_slots_link(const obs_source_t *parent, const obs_source_t *child);
void oracle_slots_set_link(obs_source_t *parent, obs_source_t *child, long long count);
void oracle_slots_refuse(obs_source_t *parent, obs_source_t *child);
size_t oracle_slots_traced(void);
void oracle_slots_event(size_t i, int *kind, obs_source_t **a, obs_source_t **b, int *result);

static void trace(int kind, obs_source_t *a, obs_source_t *b, int result)
{
	if (graph.traced < ORACLE_SLOTS_TRACE) {
		struct oracle_slots_event *e = &graph.trace[graph.traced++];
		e->kind = kind;
		e->a = a;
		e->b = b;
		e->result = result;
	}
}

static long long *refs_of(const obs_source_t *source)
{
	for (size_t i = 0; i < graph.num; i++) {
		if (graph.source[i] == source)
			return &graph.refs[i];
	}
	return NULL;
}

static size_t link_of(obs_source_t *parent, obs_source_t *child)
{
	for (size_t i = 0; i < graph.links; i++) {
		if (graph.link[i].parent == parent && graph.link[i].child == child)
			return i;
	}
	if (graph.links == ORACLE_SLOTS_LINKS)
		return ORACLE_SLOTS_LINKS;
	graph.link[graph.links].parent = parent;
	graph.link[graph.links].child = child;
	graph.link[graph.links].count = 0;
	graph.link[graph.links].refuses = false;
	return graph.links++;
}

obs_source_t *oracle_slots_source_new(int type, const char *name)
{
	obs_source_t *source;

	if (graph.num == ORACLE_SLOTS_SOURCES)
		return NULL;
	source = bzalloc(sizeof(*source));
	source->info.type = (enum obs_source_type)type;
	source->context.name = bstrdup(name);
	pthread_mutex_init(&source->transition_mutex, NULL);
	pthread_mutex_init(&source->transition_tex_mutex, NULL);
	graph.source[graph.num] = source;
	graph.refs[graph.num] = 1;
	graph.num++;
	return source;
}

/* Frees every source and forgets the graph and the trace. */
void oracle_slots_reset(void)
{
	for (size_t i = 0; i < graph.num; i++) {
		obs_source_t *source = graph.source[i];
		pthread_mutex_destroy(&source->transition_mutex);
		pthread_mutex_destroy(&source->transition_tex_mutex);
		bfree(source->context.name);
		bfree(source);
	}
	memset(&graph, 0, sizeof(graph));
}

long long oracle_slots_refs(const obs_source_t *source)
{
	long long *refs = refs_of(source);
	return refs ? *refs : -1;
}

void oracle_slots_set_refs(obs_source_t *source, long long refs)
{
	long long *r = refs_of(source);
	if (r)
		*r = refs;
}

long long oracle_slots_link(const obs_source_t *parent, const obs_source_t *child)
{
	for (size_t i = 0; i < graph.links; i++) {
		if (graph.link[i].parent == parent && graph.link[i].child == child)
			return graph.link[i].count;
	}
	return 0;
}

void oracle_slots_set_link(obs_source_t *parent, obs_source_t *child, long long count)
{
	size_t i = link_of(parent, child);
	if (i < ORACLE_SLOTS_LINKS)
		graph.link[i].count = count;
}

/* The add of child to parent fails, as for a child that contains it. */
void oracle_slots_refuse(obs_source_t *parent, obs_source_t *child)
{
	size_t i = link_of(parent, child);
	if (i < ORACLE_SLOTS_LINKS)
		graph.link[i].refuses = true;
}

size_t oracle_slots_traced(void)
{
	return graph.traced;
}

void oracle_slots_event(size_t i, int *kind, obs_source_t **a, obs_source_t **b, int *result)
{
	*kind = graph.trace[i].kind;
	*a = graph.trace[i].a;
	*b = graph.trace[i].b;
	*result = graph.trace[i].result;
}

/* The stand-ins */

obs_source_t *oracle_slots_get_ref(obs_source_t *source)
{
	long long *refs = refs_of(source);
	obs_source_t *got = NULL;

	if (refs && *refs > 0) {
		(*refs)++;
		got = source;
	}
	trace(1, source, NULL, got != NULL);
	return got;
}

void oracle_slots_release(obs_source_t *source)
{
	long long *refs = refs_of(source);

	trace(2, source, NULL, 0);
	if (refs)
		(*refs)--;
}

bool oracle_slots_add_active_child(obs_source_t *parent, obs_source_t *child)
{
	size_t i = link_of(parent, child);
	bool ok = parent != child && i < ORACLE_SLOTS_LINKS && !graph.link[i].refuses;

	trace(3, parent, child, ok);
	if (ok)
		graph.link[i].count++;
	return ok;
}

void oracle_slots_remove_active_child(obs_source_t *parent, obs_source_t *child)
{
	size_t i = link_of(parent, child);

	trace(4, parent, child, 0);
	if (i < ORACLE_SLOTS_LINKS)
		graph.link[i].count--;
}

const char *oracle_slots_get_name(const obs_source_t *source)
{
	return source ? source->context.name : NULL;
}

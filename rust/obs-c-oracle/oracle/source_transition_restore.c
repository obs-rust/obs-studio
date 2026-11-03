/* Test-only oracle: the original libobs/obs-source-transition-restore.c.h,
 * unmodified, with obs_transition_load() renamed to oracle_* so it can link
 * next to the Rust shim (rust/obs-source-seam/src/ffi/source_transition_restore.rs).
 *
 * Both sides reach the transition fields and lock through the same
 * transition seam, libobs/obs-source-transition-seam.c.h, which
 * oracle/source_transition_settings.c compiles under its own names, and
 * read the saved data through the same obs_data functions (the obs-data
 * port), as in the hybrid libobs. The rules the load asks
 * (obs_source_transition_load_*, obs_source_transition_start_child_active,
 * valid_scale_type) are the Rust ones behind the seam; blog() is
 * base-variadic.c. The size and matrix recalculation after the unlock is
 * the sizing oracle's (oracle/source_transition_size.c), so a child's size
 * comes from its stand-ins.
 *
 * The load's calls that find, link or release a source
 * (obs_get_source_by_name, obs_source_add_active_child,
 * obs_source_remove_active_child, obs_source_release) are renamed to the
 * recording stand-ins below, which the parity test hands the Rust shim too
 * (its Calls): a source is an opaque pointer with a name, a reference count,
 * an active-child link count and whether the add refuses it, and every call
 * is traced in order. */

#define obs_transition_load oracle_obs_transition_load
#define transition_recalculate_size oracle_transition_recalculate_size
#define transition_recalculate_matrices oracle_transition_recalculate_matrices
#define obs_get_source_by_name oracle_restore_get_source_by_name
#define obs_source_add_active_child oracle_restore_add_active_child
#define obs_source_remove_active_child oracle_restore_remove_active_child
#define obs_source_release oracle_restore_release

/* the restore file's precompiled header brings obs.h, obs-internal.h and
 * obs-source-transition-size.h, so the oracle_* prototypes come from them
 * through the renames */
#include "obs-source-transition-restore.c.h"

#undef obs_source_release
#undef obs_source_remove_active_child
#undef obs_source_add_active_child
#undef obs_get_source_by_name
#undef transition_recalculate_matrices
#undef transition_recalculate_size
#undef obs_transition_load

#define ORACLE_RESTORE_SOURCES 8
#define ORACLE_RESTORE_TRACE 64

/* A traced call: kind (1 obs_get_source_by_name, 2 add, 3 remove,
 * 4 release), the source it names or returned (NULL where it has none),
 * the parent of an add or remove, and an add's result. */
struct oracle_restore_event {
	int kind;
	void *source;
	void *parent;
	int result;
};

static struct {
	struct {
		void *source;
		char *name;
		long long refs;
		long long links;
		bool refuses;
	} source[ORACLE_RESTORE_SOURCES];
	size_t num;
	struct oracle_restore_event trace[ORACLE_RESTORE_TRACE];
	size_t traced;
} restore_graph;

void oracle_restore_reset(void);
bool oracle_restore_add_source(void *source, const char *name, long long refs, long long links, bool refuses);
long long oracle_restore_refs(const void *source);
long long oracle_restore_links(const void *source);
size_t oracle_restore_traced(void);
void oracle_restore_event(size_t i, int *kind, void **source, void **parent, int *result);

static void trace(int kind, void *source, void *parent, int result)
{
	if (restore_graph.traced < ORACLE_RESTORE_TRACE) {
		struct oracle_restore_event *e = &restore_graph.trace[restore_graph.traced++];
		e->kind = kind;
		e->source = source;
		e->parent = parent;
		e->result = result;
	}
}

static size_t source_index(const void *source)
{
	size_t i = 0;

	while (i < restore_graph.num && restore_graph.source[i].source != source)
		i++;
	return i;
}

void oracle_restore_reset(void)
{
	for (size_t i = 0; i < restore_graph.num; i++)
		bfree(restore_graph.source[i].name);
	memset(&restore_graph, 0, sizeof(restore_graph));
}

/* Adds a source with a name (NULL for none), its references and links and
 * whether the add refuses it; false once the graph holds 8. */
bool oracle_restore_add_source(void *source, const char *name, long long refs, long long links, bool refuses)
{
	size_t i = restore_graph.num;

	if (i == ORACLE_RESTORE_SOURCES)
		return false;
	restore_graph.source[i].source = source;
	restore_graph.source[i].name = name ? bstrdup(name) : NULL;
	restore_graph.source[i].refs = refs;
	restore_graph.source[i].links = links;
	restore_graph.source[i].refuses = refuses;
	restore_graph.num++;
	return true;
}

/* The references held on a test source, -1 for another pointer. */
long long oracle_restore_refs(const void *source)
{
	size_t i = source_index(source);
	return i < restore_graph.num ? restore_graph.source[i].refs : -1;
}

/* The active-child links held on a test source, -1 for another pointer. */
long long oracle_restore_links(const void *source)
{
	size_t i = source_index(source);
	return i < restore_graph.num ? restore_graph.source[i].links : -1;
}

size_t oracle_restore_traced(void)
{
	return restore_graph.traced;
}

void oracle_restore_event(size_t i, int *kind, void **source, void **parent, int *result)
{
	*kind = restore_graph.trace[i].kind;
	*source = restore_graph.trace[i].source;
	*parent = restore_graph.trace[i].parent;
	*result = restore_graph.trace[i].result;
}

obs_source_t *oracle_restore_get_source_by_name(const char *name)
{
	void *found = NULL;

	for (size_t i = 0; i < restore_graph.num && !found; i++) {
		if (restore_graph.source[i].name && strcmp(restore_graph.source[i].name, name) == 0) {
			restore_graph.source[i].refs++;
			found = restore_graph.source[i].source;
		}
	}
	trace(1, found, NULL, 0);
	return found;
}

bool oracle_restore_add_active_child(obs_source_t *parent, obs_source_t *child)
{
	size_t i = source_index(child);
	bool ok = i < restore_graph.num && !restore_graph.source[i].refuses;

	if (ok)
		restore_graph.source[i].links++;
	trace(2, child, parent, ok);
	return ok;
}

void oracle_restore_remove_active_child(obs_source_t *parent, obs_source_t *child)
{
	size_t i = source_index(child);

	if (i < restore_graph.num)
		restore_graph.source[i].links--;
	trace(3, child, parent, 0);
}

void oracle_restore_release(obs_source_t *source)
{
	size_t i = source_index(source);

	if (source && i < restore_graph.num)
		restore_graph.source[i].refs--;
	trace(4, source, NULL, 0);
}

/* Test-only oracle: the original libobs/obs-source-transition-size.c.h,
 * unmodified, with its five functions renamed to oracle_* so they can link
 * next to the Rust shims (rust/obs-source-seam/src/ffi/source_transition_size.rs).
 *
 * Both sides reach the transition fields and locks through the same
 * transition seam, libobs/obs-source-transition-seam.c.h, which
 * oracle/source_transition_settings.c compiles under its own names, as the
 * Rust shims call it in the hybrid libobs. The rules the file asks
 * (obs_source_transition_clock_*, obs_source_transition_render_offset) are
 * the Rust ones, as in the hybrid libobs; add_alignment(), calc_torquef()
 * and the matrix calls are the ones the test binary links (the obs-scene and
 * obs-graphics ports), which the Rust shims call too.
 *
 * The file's calls that read a child or touch a render target
 * (obs_source_get_width, obs_source_get_height, gs_texrender_reset) are
 * renamed to the recording stand-ins below, which the parity test hands
 * the Rust shims too (their Calls): a child is an opaque pointer with a
 * width and a height, a render target an opaque pointer, and every call is
 * traced in order.
 *
 * A test transition is a zeroed struct obs_source with the transition type,
 * a name and both transition mutexes. */

#define obs_transition_tick oracle_obs_transition_tick
#define transition_get_cx oracle_transition_get_cx
#define transition_get_cy oracle_transition_get_cy
#define transition_recalculate_matrices oracle_transition_recalculate_matrices
#define transition_recalculate_size oracle_transition_recalculate_size
#define obs_source_get_width oracle_size_get_width
#define obs_source_get_height oracle_size_get_height
#define gs_texrender_reset oracle_size_texrender_reset

/* the size file's precompiled header brings obs.h, graphics.h and
 * obs-source-transition-size.h, so the oracle_* prototypes come from them
 * through the renames */
#include "obs-source-transition-size.c.h"

#undef gs_texrender_reset
#undef obs_source_get_height
#undef obs_source_get_width
#undef transition_recalculate_size
#undef transition_recalculate_matrices
#undef transition_get_cy
#undef transition_get_cx
#undef obs_transition_tick

#define ORACLE_SIZE_TRANSITIONS 8
#define ORACLE_SIZE_CHILDREN 16
#define ORACLE_SIZE_TRACE 128

/* A traced call: kind (1 obs_source_get_width, 2 obs_source_get_height,
 * 3 gs_texrender_reset) and the pointer it was handed. */
struct oracle_size_event {
	int kind;
	void *a;
};

static struct {
	obs_source_t *transition[ORACLE_SIZE_TRANSITIONS];
	size_t transitions;
	struct {
		void *source;
		uint32_t cx;
		uint32_t cy;
	} child[ORACLE_SIZE_CHILDREN];
	size_t children;
	struct oracle_size_event trace[ORACLE_SIZE_TRACE];
	size_t traced;
} size_graph;

obs_source_t *oracle_size_transition_new(const char *name);
void oracle_size_reset(void);
bool oracle_size_set_child(void *source, uint32_t cx, uint32_t cy);
size_t oracle_size_traced(void);
void oracle_size_event(size_t i, int *kind, void **a);

static void trace(int kind, void *a)
{
	if (size_graph.traced < ORACLE_SIZE_TRACE) {
		struct oracle_size_event *e = &size_graph.trace[size_graph.traced++];
		e->kind = kind;
		e->a = a;
	}
}

obs_source_t *oracle_size_transition_new(const char *name)
{
	obs_source_t *source;

	if (size_graph.transitions == ORACLE_SIZE_TRANSITIONS)
		return NULL;
	source = bzalloc(sizeof(*source));
	source->info.type = OBS_SOURCE_TYPE_TRANSITION;
	source->context.name = bstrdup(name);
	pthread_mutex_init(&source->transition_mutex, NULL);
	pthread_mutex_init(&source->transition_tex_mutex, NULL);
	size_graph.transition[size_graph.transitions++] = source;
	return source;
}

void oracle_size_reset(void)
{
	for (size_t i = 0; i < size_graph.transitions; i++) {
		obs_source_t *source = size_graph.transition[i];
		pthread_mutex_destroy(&source->transition_mutex);
		pthread_mutex_destroy(&source->transition_tex_mutex);
		bfree(source->context.name);
		bfree(source);
	}
	memset(&size_graph, 0, sizeof(size_graph));
}

/* Sets a child's size, adding the child when it is new; false once 16 are
 * known. */
bool oracle_size_set_child(void *source, uint32_t cx, uint32_t cy)
{
	size_t i = 0;

	while (i < size_graph.children && size_graph.child[i].source != source)
		i++;
	if (i == ORACLE_SIZE_CHILDREN)
		return false;
	if (i == size_graph.children)
		size_graph.children++;
	size_graph.child[i].source = source;
	size_graph.child[i].cx = cx;
	size_graph.child[i].cy = cy;
	return true;
}

size_t oracle_size_traced(void)
{
	return size_graph.traced;
}

void oracle_size_event(size_t i, int *kind, void **a)
{
	*kind = size_graph.trace[i].kind;
	*a = size_graph.trace[i].a;
}

static size_t child_index(void *source)
{
	size_t i = 0;

	while (i < size_graph.children && size_graph.child[i].source != source)
		i++;
	return i;
}

uint32_t oracle_size_get_width(obs_source_t *source)
{
	size_t i = child_index(source);

	trace(1, source);
	return i < size_graph.children ? size_graph.child[i].cx : 0;
}

uint32_t oracle_size_get_height(obs_source_t *source)
{
	size_t i = child_index(source);

	trace(2, source);
	return i < size_graph.children ? size_graph.child[i].cy : 0;
}

void oracle_size_texrender_reset(gs_texrender_t *texrender)
{
	trace(3, texrender);
}

/* Test-only oracle: the original libobs/obs-source-transition-switch.c.h,
 * unmodified, with obs_transition_start() and obs_transition_set() renamed
 * to oracle_* so they can link next to the Rust shims
 * (rust/obs-source-seam/src/ffi/source_transition_switch.rs).
 *
 * Both sides reach the transition fields, lock and plugin callback through
 * the same transition seam, libobs/obs-source-transition-seam.c.h, which
 * oracle/source_transition_settings.c compiles under its own names, as the
 * Rust shims call it in the hybrid libobs; the C reads the same fields
 * directly. transition_valid() asks the Rust type check and the start rules
 * (obs_source_transition_start_*) are the Rust ones, as in the hybrid
 * libobs; blog() is base-variadic.c.
 *
 * The C's calls into the rest of libobs go to recording stand-ins, which the
 * parity test hands the Rust shims too (their Calls), so both sides run on
 * one graph, the slots oracle's (oracle/source_transition_slots.c): its
 * obs_source_get_ref(), obs_source_release(), active-child add and remove
 * and obs_source_get_name(), and the stand-ins below for
 * obs_source_enum_full_tree() (the trees the test registers),
 * obs_source_dosignal(), os_gettime_ns() (a clock that steps 1000 ns a
 * call) and transition_recalculate_size() / _matrices(). Each records its
 * call in the slots oracle's trace, in order with the others.
 *
 * obs-internal.h comes first, so its static inline obs_source_dosignal()
 * keeps its name and only the moved file's call is renamed.
 *
 * oracle_switch_plugin() gives a test transition a transition_start
 * callback that records its call and, armed by oracle_switch_reenter(),
 * makes one start or set of its own through the function the test hands
 * in (the oracle's or the Rust shim's, so each side re-enters itself). */

#include "libobs/obs-internal.h"
#include "libobs/obs-source-transition-seam.h"
#include "libobs/obs-source-transition-size.h"

bool oracle_obs_transition_start(obs_source_t *transition, enum obs_transition_mode mode, uint32_t duration_ms,
				 obs_source_t *dest);
void oracle_obs_transition_set(obs_source_t *transition, obs_source_t *source);
obs_source_t *oracle_slots_get_ref(obs_source_t *source);
void oracle_slots_release(obs_source_t *source);
bool oracle_slots_add_active_child(obs_source_t *parent, obs_source_t *child);
void oracle_slots_remove_active_child(obs_source_t *parent, obs_source_t *child);
const char *oracle_slots_get_name(const obs_source_t *source);
void oracle_slots_trace(int kind, obs_source_t *a, obs_source_t *b, int result);
void oracle_switch_enum_full_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param);
void oracle_switch_dosignal(obs_source_t *source, const char *signal_obs, const char *signal_source);
uint64_t oracle_switch_now(void);
void oracle_switch_recalculate_size(obs_source_t *transition);
void oracle_switch_recalculate_matrices(obs_source_t *transition);

#define obs_transition_start oracle_obs_transition_start
#define obs_transition_set oracle_obs_transition_set
#define obs_source_get_ref oracle_slots_get_ref
#define obs_source_release oracle_slots_release
#define obs_source_add_active_child oracle_slots_add_active_child
#define obs_source_remove_active_child oracle_slots_remove_active_child
#define obs_source_get_name oracle_slots_get_name
#define obs_source_enum_full_tree oracle_switch_enum_full_tree
#define obs_source_dosignal oracle_switch_dosignal
#define os_gettime_ns oracle_switch_now
#define transition_recalculate_size oracle_switch_recalculate_size
#define transition_recalculate_matrices oracle_switch_recalculate_matrices

#include "obs-source-transition-switch.c.h"

#undef transition_recalculate_matrices
#undef transition_recalculate_size
#undef os_gettime_ns
#undef obs_source_dosignal
#undef obs_source_enum_full_tree
#undef obs_source_get_name
#undef obs_source_remove_active_child
#undef obs_source_add_active_child
#undef obs_source_release
#undef obs_source_get_ref
#undef obs_transition_set
#undef obs_transition_start

/* The trace kinds of the stand-ins below, after the slots oracle's 1-4. */
#define ORACLE_SWITCH_WALK 5
#define ORACLE_SWITCH_SIGNAL 6
#define ORACLE_SWITCH_NOW 7
#define ORACLE_SWITCH_SIZE 8
#define ORACLE_SWITCH_MATRICES 9
#define ORACLE_SWITCH_PLUGIN 10
#define ORACLE_SWITCH_VISIT 11

#define ORACLE_SWITCH_TREE 16

typedef bool (*oracle_switch_start_fn)(obs_source_t *transition, int mode, uint32_t duration_ms, obs_source_t *dest);
typedef void (*oracle_switch_set_fn)(obs_source_t *transition, obs_source_t *source);

static struct {
	/* (root, parent, child): the walk of root visits parent and child */
	struct {
		obs_source_t *root;
		obs_source_t *parent;
		obs_source_t *child;
	} tree[ORACLE_SWITCH_TREE];
	size_t trees;
	uint64_t clock;
	/* the one re-entry the plugin makes: 0 none, 1 a start, 2 a set */
	int reenter;
	int mode;
	uint32_t duration_ms;
	obs_source_t *dest;
	oracle_switch_start_fn start;
	oracle_switch_set_fn set;
} sw;

void oracle_switch_reset(void);
void oracle_switch_tree_add(obs_source_t *root, obs_source_t *parent, obs_source_t *child);
void oracle_switch_plugin(obs_source_t *transition);
void oracle_switch_reenter(int kind, int mode, uint32_t duration_ms, obs_source_t *dest, oracle_switch_start_fn start,
			   oracle_switch_set_fn set);

/* Forgets the trees, the re-entry and the clock (the slots oracle's reset
 * frees the sources). */
void oracle_switch_reset(void)
{
	memset(&sw, 0, sizeof(sw));
	sw.clock = 1000;
}

void oracle_switch_tree_add(obs_source_t *root, obs_source_t *parent, obs_source_t *child)
{
	if (sw.trees < ORACLE_SWITCH_TREE) {
		sw.tree[sw.trees].root = root;
		sw.tree[sw.trees].parent = parent;
		sw.tree[sw.trees].child = child;
		sw.trees++;
	}
}

static void plugin_start(void *data)
{
	obs_source_t *transition = data;
	int reenter = sw.reenter;

	oracle_slots_trace(ORACLE_SWITCH_PLUGIN, transition, NULL, reenter);
	sw.reenter = 0;
	if (reenter == 1)
		sw.start(transition, sw.mode, sw.duration_ms, sw.dest);
	else if (reenter == 2)
		sw.set(transition, sw.dest);
}

/* Gives a test transition the recording transition_start callback, with
 * itself as its context data. */
void oracle_switch_plugin(obs_source_t *transition)
{
	transition->info.transition_start = plugin_start;
	transition->context.data = transition;
}

/* Arms the plugin's one re-entry: a start (kind 1) or a set (kind 2) of
 * dest, through start or set. */
void oracle_switch_reenter(int kind, int mode, uint32_t duration_ms, obs_source_t *dest, oracle_switch_start_fn start,
			   oracle_switch_set_fn set)
{
	sw.reenter = kind;
	sw.mode = mode;
	sw.duration_ms = duration_ms;
	sw.dest = dest;
	sw.start = start;
	sw.set = set;
}

/* The stand-ins */

void oracle_switch_enum_full_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	oracle_slots_trace(ORACLE_SWITCH_WALK, source, NULL, 0);
	for (size_t i = 0; i < sw.trees; i++) {
		if (sw.tree[i].root == source) {
			oracle_slots_trace(ORACLE_SWITCH_VISIT, sw.tree[i].parent, sw.tree[i].child, 0);
			enum_callback(sw.tree[i].parent, sw.tree[i].child, param);
		}
	}
}

/* Records the signal pair: 1 for the start's, 2 for the stop's, 0 else. */
void oracle_switch_dosignal(obs_source_t *source, const char *signal_obs, const char *signal_source)
{
	int which = 0;

	if (signal_obs && signal_source && strcmp(signal_obs, "source_transition_start") == 0 &&
	    strcmp(signal_source, "transition_start") == 0)
		which = 1;
	else if (signal_obs && signal_source && strcmp(signal_obs, "source_transition_stop") == 0 &&
		 strcmp(signal_source, "transition_stop") == 0)
		which = 2;
	oracle_slots_trace(ORACLE_SWITCH_SIGNAL, source, NULL, which);
}

uint64_t oracle_switch_now(void)
{
	sw.clock += 1000;
	oracle_slots_trace(ORACLE_SWITCH_NOW, NULL, NULL, 0);
	return sw.clock;
}

void oracle_switch_recalculate_size(obs_source_t *transition)
{
	oracle_slots_trace(ORACLE_SWITCH_SIZE, transition, NULL, 0);
}

void oracle_switch_recalculate_matrices(obs_source_t *transition)
{
	oracle_slots_trace(ORACLE_SWITCH_MATRICES, transition, NULL, 0);
}

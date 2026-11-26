/* Tier 3: the unchanged extracted obs_source_enum_active_sources() and
 * obs_source_enum_active_tree(), renamed oracle_*, and the production field
 * adapters the Rust shims go through (obs-source-enum-active-fields.c.h,
 * defined once, here), over the real private layout.
 *
 * Both sides reach the same recording stand-ins for obs_source_get_ref(),
 * obs_source_release() and obs_transition_enum_sources() (the original C
 * through macros, the Rust shims through their Calls), and the same scripted
 * enum_active_sources callback, which reports a case's children in order.
 * The real calls are covered by test_source_enum_active. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_enum_active_sources oracle_obs_source_enum_active_sources
#define obs_source_enum_active_tree oracle_obs_source_enum_active_tree
#define obs_source_get_ref oracle_enum_active_get_ref
#define obs_source_release oracle_enum_active_release
#define obs_transition_enum_sources oracle_enum_active_transition_enum
#include "libobs/build/libobs.pch.h"
#include "source-enum-active.inc"
#undef obs_transition_enum_sources
#undef obs_source_release
#undef obs_source_get_ref
#undef obs_source_enum_active_tree
#undef obs_source_enum_active_sources
#include "source_enum_active_fields.inc"

#define ENUM_ACTIVE_NODES 5
#define ENUM_ACTIVE_CHILDREN 3
#define ENUM_ACTIVE_EVENTS 64
#define ENUM_ACTIVE_NULL 0xfe
#define ENUM_ACTIVE_OTHER 0xff

/* Node flags. */
#define ENUM_ACTIVE_TRANSITION 1
#define ENUM_ACTIVE_CALLBACK 2
#define ENUM_ACTIVE_DATA 4
#define ENUM_ACTIVE_DYING 8

/* One call: obs_source_enum_active_sources (which 0) or _tree (1) on node
 * `root` (-1 for NULL). Each node has flags, up to three active children and
 * two transition slots (-1 for none). A child or slot at or below its node's
 * index is skipped, so every walk ends. */
struct enum_active_case {
	uint8_t which;
	int8_t root;
	uint8_t noise;
	uint8_t flags[ENUM_ACTIVE_NODES];
	int8_t children[ENUM_ACTIVE_NODES][ENUM_ACTIVE_CHILDREN];
	int8_t slots[ENUM_ACTIVE_NODES][2];
};

/* What one side did: its events, three words each (1 enum_active_sources:
 * node, whether the callback is the caller's; 2 get_ref: node; 3 release:
 * node; 4 transition enum: node, whether the callback is the caller's; 5 the
 * caller's callback: parent, child), its log lines and whether every byte of
 * the nodes is as before. */
struct enum_active_result {
	uint32_t events;
	uint64_t trace[3 * ENUM_ACTIVE_EVENTS];
	uint32_t log_count;
	int32_t log_level;
	char log[160];
	bool untouched;
};

struct enum_active_fixture {
	struct obs_source nodes[ENUM_ACTIVE_NODES];
	uint8_t script[ENUM_ACTIVE_NODES];
	const struct enum_active_case *c;
	struct enum_active_result *out;
};

static _Thread_local struct enum_active_fixture *enum_active_current;
static int enum_active_user_param;

static uint64_t enum_active_index(const void *source)
{
	struct enum_active_fixture *f = enum_active_current;
	if (!source)
		return ENUM_ACTIVE_NULL;
	for (size_t i = 0; f && i < ENUM_ACTIVE_NODES; i++) {
		if (source == &f->nodes[i])
			return i;
	}
	return ENUM_ACTIVE_OTHER;
}

static void enum_active_record(uint64_t kind, uint64_t a, uint64_t b)
{
	struct enum_active_result *out = enum_active_current->out;
	if (out->events < ENUM_ACTIVE_EVENTS) {
		out->trace[3 * out->events] = kind;
		out->trace[3 * out->events + 1] = a;
		out->trace[3 * out->events + 2] = b;
	}
	out->events++;
}

static void enum_active_user(obs_source_t *parent, obs_source_t *child, void *param)
{
	enum_active_record(5, enum_active_index(parent),
			   enum_active_index(child) | (param == &enum_active_user_param ? 0 : 0x100));
}

static uint64_t enum_active_is_user(obs_source_enum_proc_t cb, void *param)
{
	return (cb == enum_active_user ? 1 : 0) | (param == &enum_active_user_param ? 2 : 0);
}

static void enum_active_plugin(void *data, obs_source_enum_proc_t cb, void *param)
{
	struct enum_active_fixture *f = enum_active_current;
	size_t i = (size_t)((uint8_t *)data - f->script);
	enum_active_record(1, i, enum_active_is_user(cb, param));
	for (size_t k = 0; k < ENUM_ACTIVE_CHILDREN; k++) {
		int8_t child = f->c->children[i][k];
		if (child > (int8_t)i && child < ENUM_ACTIVE_NODES)
			cb(&f->nodes[i], &f->nodes[child], param);
	}
}

obs_source_t *oracle_enum_active_get_ref(obs_source_t *source);
obs_source_t *oracle_enum_active_get_ref(obs_source_t *source)
{
	uint64_t i = enum_active_index(source);
	enum_active_record(2, i, 0);
	if (i < ENUM_ACTIVE_NODES && (enum_active_current->c->flags[i] & ENUM_ACTIVE_DYING))
		return NULL;
	return source;
}

void oracle_enum_active_release(obs_source_t *source);
void oracle_enum_active_release(obs_source_t *source)
{
	enum_active_record(3, enum_active_index(source), 0);
}

void oracle_enum_active_transition_enum(obs_source_t *source, obs_source_enum_proc_t cb, void *param);
void oracle_enum_active_transition_enum(obs_source_t *source, obs_source_enum_proc_t cb, void *param)
{
	struct enum_active_fixture *f = enum_active_current;
	uint64_t i = enum_active_index(source);
	enum_active_record(4, i, enum_active_is_user(cb, param));
	if (i >= ENUM_ACTIVE_NODES)
		return;
	for (size_t k = 0; k < 2; k++) {
		int8_t slot = f->c->slots[i][k];
		if (slot > (int8_t)i && slot < ENUM_ACTIVE_NODES)
			cb(source, &f->nodes[slot], param);
	}
}

#if defined(__GNUC__) || defined(__clang__)
static void enum_active_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void enum_active_capture(int level, const char *format, va_list args, void *param)
{
	struct enum_active_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

typedef void (*enum_active_fn)(obs_source_t *, obs_source_enum_proc_t, void *);
typedef void (*enum_active_get_log)(log_handler_t *, void **);
typedef void (*enum_active_set_log)(log_handler_t, void *);

/* The original calls, for the parity test's other side. */
void oracle_source_enum_active_originals(enum_active_fn *sources, enum_active_fn *tree);
void oracle_source_enum_active_originals(enum_active_fn *sources, enum_active_fn *tree)
{
	*sources = oracle_obs_source_enum_active_sources;
	*tree = oracle_obs_source_enum_active_tree;
}

/* Runs one case through either call on fresh noise-filled nodes. False when
 * the fixture could not be built or an adapter does not reach its field. */
bool oracle_source_enum_active_run(enum_active_fn sources, enum_active_fn tree, enum_active_get_log get,
				   enum_active_set_log set, const struct enum_active_case *c,
				   struct enum_active_result *out);
bool oracle_source_enum_active_run(enum_active_fn sources, enum_active_fn tree, enum_active_get_log get,
				   enum_active_set_log set, const struct enum_active_case *c,
				   struct enum_active_result *out)
{
	struct enum_active_fixture *f = malloc(sizeof(*f));
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
	for (size_t i = 0; i < ENUM_ACTIVE_NODES; i++) {
		uint8_t flags = c->flags[i];
		f->nodes[i].context.data = (flags & ENUM_ACTIVE_DATA) ? &f->script[i] : NULL;
		f->nodes[i].info.type = (flags & ENUM_ACTIVE_TRANSITION)
						? OBS_SOURCE_TYPE_TRANSITION
						: (enum obs_source_type)(c->noise % 3 == 0 ? OBS_SOURCE_TYPE_INPUT
											   : OBS_SOURCE_TYPE_FILTER);
		f->nodes[i].info.enum_active_sources = (flags & ENUM_ACTIVE_CALLBACK) ? enum_active_plugin : NULL;
		valid = valid && obs_source_enum_active_has_data(&f->nodes[i]) == ((flags & ENUM_ACTIVE_DATA) != 0) &&
			obs_source_enum_active_is_transition(&f->nodes[i]) == ((flags & ENUM_ACTIVE_TRANSITION) != 0) &&
			obs_source_enum_active_has_callback(&f->nodes[i]) == ((flags & ENUM_ACTIVE_CALLBACK) != 0);
	}
	memcpy(before, f->nodes, sizeof(f->nodes));

	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	enum_active_current = f;
	set(enum_active_capture, out);
	obs_source_t *root = c->root < 0 || c->root >= ENUM_ACTIVE_NODES ? NULL : &f->nodes[c->root];
	(c->which == 0 ? sources : tree)(root, enum_active_user, &enum_active_user_param);
	set(old_handler, old_param);
	enum_active_current = NULL;

	out->untouched = memcmp(before, f->nodes, sizeof(f->nodes)) == 0;
	free(before);
	free(f);
	return valid;
}

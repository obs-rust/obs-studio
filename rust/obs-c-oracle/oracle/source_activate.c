/* Tier 3: the unchanged extracted obs_source_activate() and
 * obs_source_deactivate() with their four tree callbacks
 * (libobs/obs-source-activate.c.h), renamed oracle_*, and the production
 * counter adapters the Rust shims call (obs-source-activate-fields.c.h,
 * compiled here under saf_real_* names), over the real private layout.
 *
 * This file holds the one definition, for the test binaries, of each
 * counter adapter and of obs_source_enum_active_tree(). With a fixture of
 * this file installed on the calling thread, the adapters are the real
 * ones, and the walk is a scripted one that both sides reach: it records
 * the root and every node's counters, then reports the case's edges in
 * order to the callback it was given (the original's static callbacks, or
 * the Rust ones). Without one, the adapters report to oracle/view.c's
 * activation trace for the view, canvas, show-counter and active-child
 * tests' sources (they never touch those), and the walk does nothing. The
 * real walk and counters are covered by test_source_activate. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_activate oracle_obs_source_activate
#define obs_source_deactivate oracle_obs_source_deactivate
#include "libobs/build/libobs.pch.h"
#include "source-activate.inc"
#undef obs_source_deactivate
#undef obs_source_activate

#define obs_source_activate_inc_show_refs saf_real_inc_show_refs
#define obs_source_activate_dec_show_refs saf_real_dec_show_refs
#define obs_source_activate_load_show_refs saf_real_load_show_refs
#define obs_source_activate_inc_activate_refs saf_real_inc_activate_refs
#define obs_source_activate_dec_activate_refs saf_real_dec_activate_refs
#define obs_source_activate_load_activate_refs saf_real_load_activate_refs
#include "source_activate_fields.inc"
#undef obs_source_activate_load_activate_refs
#undef obs_source_activate_dec_activate_refs
#undef obs_source_activate_inc_activate_refs
#undef obs_source_activate_load_show_refs
#undef obs_source_activate_dec_show_refs
#undef obs_source_activate_inc_show_refs

#define SAF_NODES 4
#define SAF_EDGES 8
#define SAF_EVENTS 8
/* An index for NULL, and for a pointer that is no node. */
#define SAF_NULL 0xfe
#define SAF_OTHER 0xff

/* One call: activate (which 0) or deactivate (1) of node `source` (-1 for
 * NULL) for `view_type`, from the given counters, on nodes whose every
 * other byte is `noise`. Each walk reports `edges` edges, each between two
 * nodes (a parent of -1 is NULL; children are always nodes). */
struct saf_case {
	uint8_t which;
	int8_t source;
	uint8_t noise;
	uint32_t view_type;
	long show[SAF_NODES];
	long active[SAF_NODES];
	uint8_t edges;
	int8_t edge_parent[SAF_EDGES];
	int8_t edge_child[SAF_EDGES];
};

/* What one side did: each walk in order (its root, and every node's
 * show_refs and activate_refs when it began), the counters afterwards, its
 * log lines (count, level of the last, the last), and whether every other
 * byte of the nodes is as before. */
struct saf_result {
	uint32_t events;
	int64_t trace[SAF_EVENTS][1 + 2 * SAF_NODES];
	long show[SAF_NODES];
	long active[SAF_NODES];
	uint32_t log_count;
	int32_t log_level;
	char log[128];
	bool untouched;
};

struct saf_fixture {
	struct obs_source nodes[SAF_NODES];
	const struct saf_case *c;
	struct saf_result *out;
};

/* Per thread: the Rust test harness runs cases on several threads. */
static _Thread_local struct saf_fixture *saf_current;

static int64_t saf_index(const void *source)
{
	struct saf_fixture *f = saf_current;
	if (!source)
		return SAF_NULL;
	for (size_t i = 0; i < SAF_NODES; i++) {
		if (source == &f->nodes[i])
			return (int64_t)i;
	}
	return SAF_OTHER;
}

static obs_source_t *saf_node(int8_t index)
{
	return index < 0 || index >= SAF_NODES ? NULL : &saf_current->nodes[index];
}

/* ------------------------------------------------------------------------- */
/* the one definition of each adapter and of the walk */

void oracle_view_activation_step(const obs_source_t *source, bool deactivate, bool active);

void obs_source_activate_inc_show_refs(obs_source_t *source);
void obs_source_activate_inc_show_refs(obs_source_t *source)
{
	if (saf_current)
		saf_real_inc_show_refs(source);
	else
		oracle_view_activation_step(source, false, false);
}

void obs_source_activate_dec_show_refs(obs_source_t *source);
void obs_source_activate_dec_show_refs(obs_source_t *source)
{
	/* Never reached without a fixture: the loads below answer 0. */
	if (saf_current)
		saf_real_dec_show_refs(source);
}

long obs_source_activate_load_show_refs(const obs_source_t *source);
long obs_source_activate_load_show_refs(const obs_source_t *source)
{
	if (saf_current)
		return saf_real_load_show_refs(source);
	oracle_view_activation_step(source, true, false);
	return 0;
}

void obs_source_activate_inc_activate_refs(obs_source_t *source);
void obs_source_activate_inc_activate_refs(obs_source_t *source)
{
	if (saf_current)
		saf_real_inc_activate_refs(source);
	else
		oracle_view_activation_step(source, false, true);
}

void obs_source_activate_dec_activate_refs(obs_source_t *source);
void obs_source_activate_dec_activate_refs(obs_source_t *source)
{
	if (saf_current)
		saf_real_dec_activate_refs(source);
}

long obs_source_activate_load_activate_refs(const obs_source_t *source);
long obs_source_activate_load_activate_refs(const obs_source_t *source)
{
	if (saf_current)
		return saf_real_load_activate_refs(source);
	oracle_view_activation_step(source, true, true);
	return 0;
}

/* Also linked by the test binaries that run the audio tick's shim (which
 * names it, #5401) and never reach it. */
void obs_source_enum_active_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	struct saf_fixture *f = saf_current;
	if (!f)
		return;
	struct saf_result *out = f->out;
	if (out->events < SAF_EVENTS) {
		int64_t *e = out->trace[out->events];
		e[0] = saf_index(source);
		for (size_t i = 0; i < SAF_NODES; i++) {
			e[1 + i] = f->nodes[i].show_refs;
			e[1 + SAF_NODES + i] = f->nodes[i].activate_refs;
		}
	}
	out->events++;
	for (uint8_t i = 0; i < f->c->edges && i < SAF_EDGES; i++)
		enum_callback(saf_node(f->c->edge_parent[i]), saf_node(f->c->edge_child[i]), param);
}

/* ------------------------------------------------------------------------- */
/* the parity run */

#if defined(__GNUC__) || defined(__clang__)
static void saf_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void saf_capture(int level, const char *format, va_list args, void *param)
{
	struct saf_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

typedef void (*saf_call)(obs_source_t *, uint32_t);
typedef void (*saf_get_log)(log_handler_t *, void **);
typedef void (*saf_set_log)(log_handler_t, void *);

static void saf_original_activate(obs_source_t *source, uint32_t view_type)
{
	oracle_obs_source_activate(source, (enum view_type)view_type);
}

static void saf_original_deactivate(obs_source_t *source, uint32_t view_type)
{
	oracle_obs_source_deactivate(source, (enum view_type)view_type);
}

/* The original calls, for the parity test's other side. */
void oracle_source_activate_originals(saf_call *activate, saf_call *deactivate);
void oracle_source_activate_originals(saf_call *activate, saf_call *deactivate)
{
	*activate = saf_original_activate;
	*deactivate = saf_original_deactivate;
}

/* Runs one case through `activate` or `deactivate` on fresh noise-filled
 * nodes. False when the fixture could not be built or an adapter does not
 * reach its field. */
bool oracle_source_activate_run(saf_call activate, saf_call deactivate, saf_get_log get, saf_set_log set,
				const struct saf_case *c, struct saf_result *out);
bool oracle_source_activate_run(saf_call activate, saf_call deactivate, saf_get_log get, saf_set_log set,
				const struct saf_case *c, struct saf_result *out)
{
	struct saf_fixture *f = malloc(sizeof(*f));
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
	for (size_t i = 0; i < SAF_NODES; i++) {
		f->nodes[i].show_refs = c->show[i];
		f->nodes[i].activate_refs = c->active[i];
		/* The real adapters reach the fields they name. */
		valid = valid && saf_real_load_show_refs(&f->nodes[i]) == c->show[i] &&
			saf_real_load_activate_refs(&f->nodes[i]) == c->active[i];
	}
	memcpy(before, f->nodes, sizeof(f->nodes));

	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	saf_current = f;
	set(saf_capture, out);
	obs_source_t *source = saf_node(c->source);
	if (c->which == 0)
		activate(source, c->view_type);
	else
		deactivate(source, c->view_type);
	set(old_handler, old_param);
	saf_current = NULL;

	for (size_t i = 0; i < SAF_NODES; i++) {
		out->show[i] = f->nodes[i].show_refs;
		out->active[i] = f->nodes[i].activate_refs;
		before[i].show_refs = f->nodes[i].show_refs;
		before[i].activate_refs = f->nodes[i].activate_refs;
	}
	out->untouched = memcmp(before, f->nodes, sizeof(f->nodes)) == 0;
	free(before);
	free(f);
	return valid;
}

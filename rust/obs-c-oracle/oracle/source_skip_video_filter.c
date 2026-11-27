/* Tier 3: the unchanged extracted obs_source_skip_video_filter()
 * (libobs/obs-source-skip-video-filter.c.h), renamed oracle_*, and the
 * production adapters the Rust shim calls
 * (obs-source-skip-video-filter-fields.c.h, under their real names, the one
 * definition of each in the test binaries), over the real private layout of
 * noise-filled sources.
 *
 * Both compile against the same recording stand-ins for every call they
 * make (the target and parent lookups and the five draws): each records the
 * node it is given as an index and answers from the case. The parent's
 * output flags, the target's video_render and its deinterlace mode are real
 * fields. The log line goes through the real blog(), captured by a handler
 * installed for the call. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

#define SSKF_NODES 3
#define SSKF_EVENTS 8
/* An index for NULL, and for a pointer that is none of the case's. */
#define SSKF_NULL 0xfe
#define SSKF_OTHER 0xff

/* One call of the skip on node `filter` (-1: NULL). The lookups answer
 * node `target` and node `parent` (-1: NULL). Each node has its output
 * flags, a video_render or none, and a deinterlace mode. */
struct sskf_case {
	int8_t filter;
	int8_t target;
	int8_t parent;
	uint8_t noise;
	uint32_t flags[SSKF_NODES];
	bool video_render[SSKF_NODES];
	int32_t deinterlace_mode[SSKF_NODES];
};

/* What one side did: each call in order as {kind, node}, its log lines
 * (count, level of the last, the last), and whether every byte of the nodes
 * is as before. */
struct sskf_result {
	uint32_t events;
	int64_t trace[SSKF_EVENTS][2];
	uint32_t log_count;
	int32_t log_level;
	char log[128];
	bool untouched;
};

struct sskf_fixture {
	struct obs_source nodes[SSKF_NODES];
	const struct sskf_case *c;
	struct sskf_result *out;
};

/* Per thread: the Rust test harness runs cases on several threads. */
static _Thread_local struct sskf_fixture *sskf_current;

static int64_t sskf_node_index(const void *source)
{
	struct sskf_fixture *f = sskf_current;
	if (!source)
		return SSKF_NULL;
	for (size_t i = 0; i < SSKF_NODES; i++) {
		if (source == &f->nodes[i])
			return (int64_t)i;
	}
	return SSKF_OTHER;
}

static obs_source_t *sskf_node(int8_t index)
{
	return index < 0 || index >= SSKF_NODES ? NULL : &sskf_current->nodes[index];
}

static void sskf_event(int64_t kind, const void *source)
{
	struct sskf_result *out = sskf_current->out;
	if (out->events < SSKF_EVENTS) {
		out->trace[out->events][0] = kind;
		out->trace[out->events][1] = sskf_node_index(source);
	}
	out->events++;
}

/* A video_render to point at; never called. */
static void sskf_video_render_cb(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(effect);
}

/* ------------------------------------------------------------------------- */
/* the stand-ins */

obs_source_t *sskf_get_target(const obs_source_t *filter);
obs_source_t *sskf_get_target(const obs_source_t *filter)
{
	sskf_event('T', filter);
	return sskf_node(sskf_current->c->target);
}

obs_source_t *sskf_get_parent(const obs_source_t *filter);
obs_source_t *sskf_get_parent(const obs_source_t *filter)
{
	sskf_event('P', filter);
	return sskf_node(sskf_current->c->parent);
}

void sskf_default(obs_source_t *source);
void sskf_default(obs_source_t *source)
{
	sskf_event('D', source);
}

void sskf_main(obs_source_t *source);
void sskf_main(obs_source_t *source)
{
	sskf_event('M', source);
}

void sskf_deinterlace(obs_source_t *source);
void sskf_deinterlace(obs_source_t *source)
{
	sskf_event('I', source);
}

void sskf_async(obs_source_t *source);
void sskf_async(obs_source_t *source)
{
	sskf_event('A', source);
}

void sskf_video_render(obs_source_t *source);
void sskf_video_render(obs_source_t *source)
{
	sskf_event('V', source);
}

/* ------------------------------------------------------------------------- */
/* the original and the production adapters, over the stand-ins */

#define obs_filter_get_target sskf_get_target
#define obs_filter_get_parent sskf_get_parent
#define obs_source_default_render sskf_default
#define obs_source_skip_video_filter_main_render sskf_main
#define deinterlace_render sskf_deinterlace
#define obs_source_skip_video_filter_async_video sskf_async
#define obs_source_video_render sskf_video_render
#define obs_source_skip_video_filter oracle_obs_source_skip_video_filter
void oracle_obs_source_skip_video_filter(obs_source_t *filter);
#include "source-skip-video-filter.inc"
#undef obs_source_skip_video_filter
#include "source_skip_video_filter_fields.inc"
#undef obs_source_video_render
#undef obs_source_skip_video_filter_async_video
#undef deinterlace_render
#undef obs_source_skip_video_filter_main_render
#undef obs_source_default_render
#undef obs_filter_get_parent
#undef obs_filter_get_target

/* ------------------------------------------------------------------------- */
/* the parity run */

#if defined(__GNUC__) || defined(__clang__)
static void sskf_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void sskf_capture(int level, const char *format, va_list args, void *param)
{
	struct sskf_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

typedef void (*sskf_skip)(obs_source_t *);
typedef void (*sskf_get_log)(log_handler_t *, void **);
typedef void (*sskf_set_log)(log_handler_t, void *);

/* The original, for the parity test's other side. */
sskf_skip oracle_source_skip_video_filter_original(void);
sskf_skip oracle_source_skip_video_filter_original(void)
{
	return oracle_obs_source_skip_video_filter;
}

/* Runs one case through `skip` on fresh noise-filled nodes. False when the
 * fixture could not be built or an adapter does not reach its field. */
bool oracle_source_skip_video_filter_run(sskf_skip skip, sskf_get_log get, sskf_set_log set, const struct sskf_case *c,
					 struct sskf_result *out);
bool oracle_source_skip_video_filter_run(sskf_skip skip, sskf_get_log get, sskf_set_log set, const struct sskf_case *c,
					 struct sskf_result *out)
{
	struct sskf_fixture *f = malloc(sizeof(*f));
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
	for (size_t i = 0; i < SSKF_NODES; i++) {
		f->nodes[i].info.output_flags = c->flags[i];
		f->nodes[i].info.video_render = c->video_render[i] ? sskf_video_render_cb : NULL;
		f->nodes[i].deinterlace_mode = (enum obs_deinterlace_mode)c->deinterlace_mode[i];
	}
	sskf_current = f;

	/* The real adapters reach the fields they name. */
	bool valid = true;
	for (size_t i = 0; i < SSKF_NODES; i++) {
		valid = valid && obs_source_skip_video_filter_parent_flags(&f->nodes[i]) == c->flags[i] &&
			obs_source_skip_video_filter_has_video_render(&f->nodes[i]) == c->video_render[i] &&
			obs_source_skip_video_filter_deinterlacing(&f->nodes[i]) == (c->deinterlace_mode[i] != 0);
	}
	memcpy(before, f->nodes, sizeof(f->nodes));

	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	set(sskf_capture, out);
	skip(sskf_node(c->filter));
	set(old_handler, old_param);
	sskf_current = NULL;

	out->untouched = memcmp(before, f->nodes, sizeof(f->nodes)) == 0;
	free(before);
	free(f);
	return valid;
}

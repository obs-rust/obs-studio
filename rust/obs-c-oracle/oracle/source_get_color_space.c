/* Tier 3: the unchanged extracted obs_source_get_color_space(), renamed
 * oracle_*, with its routing calls bound to the original C routing
 * (oracle/source_color_space.c), and the production field adapters the Rust
 * shims go through (obs-source-get-color-space-fields.c.h, defined once,
 * here), over the real private layout.
 *
 * A case is up to GET_CS_NODES sources linked to higher indices only (no
 * cycles: the C recurses without end on one), each with its routing fields,
 * async format and transfer, and an optional callback that records which
 * source asked it with which list and answers that source's value. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_color_space oracle_obs_source_get_color_space
#define obs_source_color_space_route oracle_obs_source_color_space_route
#define obs_source_color_space_pick oracle_obs_source_color_space_pick
#include "libobs/build/libobs.pch.h"
#include "source-get-color-space.inc"
#undef obs_source_color_space_pick
#undef obs_source_color_space_route
#undef obs_source_get_color_space
#include "source_get_color_space_fields.inc"

#define GET_CS_NODES 4
#define GET_CS_LIST 4
#define GET_CS_CALLS 8

struct get_cs_case {
	int8_t start; /* -1: NULL */
	uint8_t noise;
	uint8_t count;
	bool null_list;
	int32_t list[GET_CS_LIST];
	int32_t type[GET_CS_NODES];
	uint32_t flags[GET_CS_NODES];
	bool has_data[GET_CS_NODES];
	bool enabled[GET_CS_NODES];
	int8_t parent[GET_CS_NODES]; /* -1: NULL */
	int8_t target[GET_CS_NODES];
	int32_t format[GET_CS_NODES];
	int32_t trc[GET_CS_NODES];
	bool has_callback[GET_CS_NODES];
	int32_t answer[GET_CS_NODES];
};

struct get_cs_result {
	int32_t returned;
	uint32_t calls;
	/* per callback call: the source index, the count, whether the list
	 * pointer was the caller's */
	int32_t call_source[GET_CS_CALLS];
	uint64_t call_count[GET_CS_CALLS];
	bool call_list[GET_CS_CALLS];
	uint32_t log_count;
	int32_t log_level;
	char log[160];
	bool untouched;
};

struct get_cs_fixture {
	struct obs_source nodes[GET_CS_NODES];
	int data[GET_CS_NODES];
};

static struct get_cs_result *get_cs_out;
static const enum gs_color_space *get_cs_list;

static enum gs_color_space get_cs_callback(void *data, size_t count, const enum gs_color_space *preferred_spaces)
{
	int index = *(int *)data;
	struct get_cs_result *out = get_cs_out;
	if (out->calls < GET_CS_CALLS) {
		out->call_source[out->calls] = index;
		out->call_count[out->calls] = (uint64_t)count;
		out->call_list[out->calls] = preferred_spaces == get_cs_list;
	}
	out->calls++;
	return (enum gs_color_space)(index + 100);
}

#if defined(__GNUC__) || defined(__clang__)
static void get_cs_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void get_cs_capture(int level, const char *format, va_list args, void *param)
{
	struct get_cs_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

typedef enum gs_color_space (*get_cs_fn)(obs_source_t *, size_t, const enum gs_color_space *);
typedef void (*get_cs_get_log)(log_handler_t *, void **);
typedef void (*get_cs_set_log)(log_handler_t, void *);

get_cs_fn oracle_source_get_color_space_original(void);
get_cs_fn oracle_source_get_color_space_original(void)
{
	return oracle_obs_source_get_color_space;
}

static obs_source_t *get_cs_node(struct get_cs_fixture *f, int8_t i)
{
	return i >= 0 && i < GET_CS_NODES ? &f->nodes[i] : NULL;
}

/* Runs one case through `fn` on fresh noise-filled sources. False when the
 * fixture could not be built. */
bool oracle_source_get_color_space_run(get_cs_fn fn, get_cs_get_log get, get_cs_set_log set,
				       const struct get_cs_case *c, struct get_cs_result *out);
bool oracle_source_get_color_space_run(get_cs_fn fn, get_cs_get_log get, get_cs_set_log set,
				       const struct get_cs_case *c, struct get_cs_result *out)
{
	struct get_cs_fixture *f = malloc(sizeof(*f));
	struct obs_source *before = malloc(sizeof(f->nodes));
	enum gs_color_space list[GET_CS_LIST];
	if (!f || !before) {
		free(f);
		free(before);
		return false;
	}
	memset(f, c->noise, sizeof(*f));
	memset(out, 0, sizeof(*out));
	out->log_level = -1;
	for (size_t i = 0; i < GET_CS_LIST; i++)
		list[i] = (enum gs_color_space)c->list[i];
	for (int i = 0; i < GET_CS_NODES; i++) {
		struct obs_source *s = &f->nodes[i];
		f->data[i] = i;
		s->info.type = (enum obs_source_type)c->type[i];
		s->info.output_flags = c->flags[i];
		s->context.data = c->has_data[i] ? &f->data[i] : NULL;
		s->enabled = c->enabled[i];
		s->filter_parent = get_cs_node(f, c->parent[i]);
		s->filter_target = get_cs_node(f, c->target[i]);
		s->async_format = (enum video_format)c->format[i];
		s->async_trc = (enum video_trc)c->trc[i];
		s->info.video_get_color_space = c->has_callback[i] ? get_cs_callback : NULL;
	}
	memcpy(before, f->nodes, sizeof(f->nodes));

	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	get_cs_out = out;
	get_cs_list = c->null_list ? NULL : list;
	set(get_cs_capture, out);
	out->returned = (int32_t)fn(get_cs_node(f, c->start), c->count, get_cs_list);
	set(old_handler, old_param);
	get_cs_out = NULL;
	get_cs_list = NULL;

	out->untouched = memcmp(before, f->nodes, sizeof(f->nodes)) == 0;
	free(before);
	free(f);
	return true;
}

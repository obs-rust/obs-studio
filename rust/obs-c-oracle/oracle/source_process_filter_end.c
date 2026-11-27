/* Tier 3: the unchanged extracted obs_source_process_filter_end() and
 * obs_source_process_filter_tech_end()
 * (libobs/obs-source-process-filter-end.c.h), renamed oracle_*, and the
 * production adapters the Rust shim calls
 * (obs-source-process-filter-end-fields.c.h, under their real names, the one
 * definition of each in the test binaries), over the real private layout of
 * noise-filled sources.
 *
 * Both compile against the same recording stand-ins for every call they
 * make (the target and parent lookups, gs_set_linear_srgb(), the texrender's
 * texture and the two draws): each records what it is given and answers
 * from the case. The filter's bypass flag, output flags and texrender are
 * real fields. The log line goes through the real blog(), captured by a
 * handler installed for the call. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

#define SPFE_NODES 3
#define SPFE_EVENTS 12
/* An index for NULL, and for a pointer that is none of the case's. */
#define SPFE_NULL 0xfe
#define SPFE_OTHER 0xff

/* The technique names a case can pass, and how a call's name is recorded:
 * 0 NULL, 1 "Draw", 2 "DrawAlphaDivide", 3 "", 4 any other. */
static const char *const spfe_techs[] = {NULL, "Draw", "DrawAlphaDivide", ""};

/* One call on node `filter` (-1: NULL) with `effect` (0: NULL, 1: the
 * fixture's), `width`, `height` and technique `tech` (an index into
 * spfe_techs); `end` false for _tech_end, true for _end. The lookups answer
 * node `target` and node `parent` (-1: NULL); the texture lookup answers the
 * fixture's texture when `texture` is set. Each node has its output flags,
 * bypass flag and a texrender or none. Linear sRGB starts as `linear`. */
struct spfe_case {
	int8_t filter;
	int8_t target;
	int8_t parent;
	uint8_t noise;
	bool end;
	uint8_t effect;
	uint8_t tech;
	bool texture;
	bool linear;
	uint32_t width;
	uint32_t height;
	uint32_t flags[SPFE_NODES];
	bool bypass[SPFE_NODES];
	bool texrender[SPFE_NODES];
};

/* What one side did: each call in order as {kind, a, b, c, d}, its log
 * lines (count, level of the last, the last), each node's bypass flag
 * after, linear sRGB after, and whether every other byte of the nodes is as
 * before. */
struct spfe_result {
	uint32_t events;
	int64_t trace[SPFE_EVENTS][5];
	uint32_t log_count;
	int32_t log_level;
	char log[128];
	bool bypass[SPFE_NODES];
	bool linear;
	bool untouched;
};

struct spfe_fixture {
	struct obs_source nodes[SPFE_NODES];
	char texrenders[SPFE_NODES];
	char effect;
	char texture;
	bool linear;
	const struct spfe_case *c;
	struct spfe_result *out;
};

/* Per thread: the Rust test harness runs cases on several threads. */
static _Thread_local struct spfe_fixture *spfe_current;

static int64_t spfe_node_index(const void *source)
{
	struct spfe_fixture *f = spfe_current;
	if (!source)
		return SPFE_NULL;
	for (size_t i = 0; i < SPFE_NODES; i++) {
		if (source == &f->nodes[i])
			return (int64_t)i;
	}
	return SPFE_OTHER;
}

static int64_t spfe_texrender_index(const void *texrender)
{
	struct spfe_fixture *f = spfe_current;
	if (!texrender)
		return SPFE_NULL;
	for (size_t i = 0; i < SPFE_NODES; i++) {
		if (texrender == &f->texrenders[i])
			return (int64_t)i;
	}
	return SPFE_OTHER;
}

static int64_t spfe_effect_index(const void *effect)
{
	if (!effect)
		return 0;
	return effect == &spfe_current->effect ? 1 : SPFE_OTHER;
}

static int64_t spfe_texture_index(const void *texture)
{
	if (!texture)
		return SPFE_NULL;
	return texture == &spfe_current->texture ? 1 : SPFE_OTHER;
}

static int64_t spfe_tech_index(const char *tech)
{
	if (!tech)
		return 0;
	for (size_t i = 1; i < sizeof(spfe_techs) / sizeof(spfe_techs[0]); i++) {
		if (strcmp(tech, spfe_techs[i]) == 0)
			return (int64_t)i;
	}
	return 4;
}

static obs_source_t *spfe_node(int8_t index)
{
	return index < 0 || index >= SPFE_NODES ? NULL : &spfe_current->nodes[index];
}

static void spfe_event(int64_t kind, int64_t a, int64_t b, int64_t c, int64_t d)
{
	struct spfe_result *out = spfe_current->out;
	if (out->events < SPFE_EVENTS) {
		out->trace[out->events][0] = kind;
		out->trace[out->events][1] = a;
		out->trace[out->events][2] = b;
		out->trace[out->events][3] = c;
		out->trace[out->events][4] = d;
	}
	out->events++;
}

/* ------------------------------------------------------------------------- */
/* the stand-ins */

obs_source_t *spfe_get_target(const obs_source_t *filter);
obs_source_t *spfe_get_target(const obs_source_t *filter)
{
	spfe_event('T', spfe_node_index(filter), 0, 0, 0);
	return spfe_node(spfe_current->c->target);
}

obs_source_t *spfe_get_parent(const obs_source_t *filter);
obs_source_t *spfe_get_parent(const obs_source_t *filter)
{
	spfe_event('P', spfe_node_index(filter), 0, 0, 0);
	return spfe_node(spfe_current->c->parent);
}

bool spfe_set_linear_srgb(bool linear_srgb);
bool spfe_set_linear_srgb(bool linear_srgb)
{
	spfe_event('L', linear_srgb, 0, 0, 0);
	const bool previous = spfe_current->linear;
	spfe_current->linear = linear_srgb;
	return previous;
}

gs_texture_t *spfe_texrender_get_texture(const gs_texrender_t *texrender);
gs_texture_t *spfe_texrender_get_texture(const gs_texrender_t *texrender)
{
	spfe_event('X', spfe_texrender_index(texrender), 0, 0, 0);
	return texrender && spfe_current->c->texture ? (gs_texture_t *)&spfe_current->texture : NULL;
}

void spfe_render_bypass(obs_source_t *target, gs_effect_t *effect, const char *tech_name);
void spfe_render_bypass(obs_source_t *target, gs_effect_t *effect, const char *tech_name)
{
	spfe_event('B', spfe_node_index(target), spfe_effect_index(effect), spfe_tech_index(tech_name), 0);
}

void spfe_render_tex(gs_texture_t *tex, gs_effect_t *effect, uint32_t width, uint32_t height, const char *tech_name);
void spfe_render_tex(gs_texture_t *tex, gs_effect_t *effect, uint32_t width, uint32_t height, const char *tech_name)
{
	spfe_event('R', spfe_texture_index(tex), spfe_effect_index(effect), spfe_tech_index(tech_name),
		   (int64_t)width << 32 | height);
}

/* ------------------------------------------------------------------------- */
/* the original and the production adapters, over the stand-ins */

#define obs_filter_get_target spfe_get_target
#define obs_filter_get_parent spfe_get_parent
#define gs_set_linear_srgb spfe_set_linear_srgb
#define gs_texrender_get_texture spfe_texrender_get_texture
#define obs_source_process_filter_end_render_bypass spfe_render_bypass
#define obs_source_process_filter_end_render_tex spfe_render_tex
#define obs_source_process_filter_tech_end oracle_obs_source_process_filter_tech_end
#define obs_source_process_filter_end oracle_obs_source_process_filter_end
void oracle_obs_source_process_filter_tech_end(obs_source_t *filter, gs_effect_t *effect, uint32_t width,
					       uint32_t height, const char *tech_name);
void oracle_obs_source_process_filter_end(obs_source_t *filter, gs_effect_t *effect, uint32_t width, uint32_t height);
#include "source-process-filter-end.inc"
#undef obs_source_process_filter_end
#undef obs_source_process_filter_tech_end
#include "source_process_filter_end_fields.inc"
#undef obs_source_process_filter_end_render_tex
#undef obs_source_process_filter_end_render_bypass
#undef gs_texrender_get_texture
#undef gs_set_linear_srgb
#undef obs_filter_get_parent
#undef obs_filter_get_target

/* ------------------------------------------------------------------------- */
/* the parity run */

#if defined(__GNUC__) || defined(__clang__)
static void spfe_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void spfe_capture(int level, const char *format, va_list args, void *param)
{
	struct spfe_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

typedef void (*spfe_tech_end)(obs_source_t *, gs_effect_t *, uint32_t, uint32_t, const char *);
typedef void (*spfe_end)(obs_source_t *, gs_effect_t *, uint32_t, uint32_t);
typedef void (*spfe_get_log)(log_handler_t *, void **);
typedef void (*spfe_set_log)(log_handler_t, void *);

/* The originals, for the parity test's other side. */
spfe_tech_end oracle_source_process_filter_tech_end_original(void);
spfe_tech_end oracle_source_process_filter_tech_end_original(void)
{
	return oracle_obs_source_process_filter_tech_end;
}

spfe_end oracle_source_process_filter_end_original(void);
spfe_end oracle_source_process_filter_end_original(void)
{
	return oracle_obs_source_process_filter_end;
}

/* Runs one case through `tech_end` (c->end false) or `end` (c->end true)
 * on fresh noise-filled nodes. False when the fixture could not be built or
 * an adapter does not reach its field. */
bool oracle_source_process_filter_end_run(spfe_tech_end tech_end, spfe_end end, spfe_get_log get, spfe_set_log set,
					  const struct spfe_case *c, struct spfe_result *out);
bool oracle_source_process_filter_end_run(spfe_tech_end tech_end, spfe_end end, spfe_get_log get, spfe_set_log set,
					  const struct spfe_case *c, struct spfe_result *out)
{
	struct spfe_fixture *f = malloc(sizeof(*f));
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
	f->linear = c->linear;
	for (size_t i = 0; i < SPFE_NODES; i++) {
		f->nodes[i].info.output_flags = c->flags[i];
		f->nodes[i].filter_bypass_active = c->bypass[i];
		f->nodes[i].filter_texrender = c->texrender[i] ? (gs_texrender_t *)&f->texrenders[i] : NULL;
	}
	spfe_current = f;

	/* The real adapters reach the fields they name. */
	bool valid = true;
	for (size_t i = 0; i < SPFE_NODES; i++) {
		valid = valid && obs_source_process_filter_end_filter_flags(&f->nodes[i]) == c->flags[i] &&
			obs_source_process_filter_end_bypass_active(&f->nodes[i]) == c->bypass[i];
	}
	memcpy(before, f->nodes, sizeof(f->nodes));

	gs_effect_t *effect = c->effect ? (gs_effect_t *)&f->effect : NULL;
	const char *tech = c->tech < sizeof(spfe_techs) / sizeof(spfe_techs[0]) ? spfe_techs[c->tech] : NULL;

	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	set(spfe_capture, out);
	if (c->end)
		end(spfe_node(c->filter), effect, c->width, c->height);
	else
		tech_end(spfe_node(c->filter), effect, c->width, c->height, tech);
	set(old_handler, old_param);
	spfe_current = NULL;

	/* The bypass flags are reported; every other byte must be as before. */
	for (size_t i = 0; i < SPFE_NODES; i++) {
		out->bypass[i] = f->nodes[i].filter_bypass_active;
		before[i].filter_bypass_active = f->nodes[i].filter_bypass_active;
	}
	out->linear = f->linear;
	out->untouched = memcmp(before, f->nodes, sizeof(f->nodes)) == 0;
	free(before);
	free(f);
	return valid;
}

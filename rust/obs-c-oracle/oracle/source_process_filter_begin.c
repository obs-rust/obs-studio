/* Tier 3: the unchanged extracted obs_source_process_filter_begin() and
 * obs_source_process_filter_begin_with_color_space()
 * (libobs/obs-source-process-filter-begin.c.h), renamed oracle_*, and the
 * production adapters the Rust shim calls
 * (obs-source-process-filter-begin-fields.c.h, under their real names, the
 * one definition of each in the test binaries), over the real private
 * layout of noise-filled sources.
 *
 * Both compile against the same recording stand-ins for every call they
 * make (the target and parent lookups, the base size, the current color
 * space, the skip, the texrender's format, destroy, create, begin and end,
 * the blend state, clear and ortho, and the two draws of the target): each
 * records what it is given and answers from the case. The filter's bypass
 * flag, allow_direct, texrender and name and each node's output flags are
 * real fields. The log lines go through the real blog(), captured by a
 * handler installed for the call. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

#define SPFB_NODES 3
/* The node texrenders, and one more for gs_texrender_create(). */
#define SPFB_TEXRENDERS (SPFB_NODES + 1)
#define SPFB_EVENTS 24
/* An index for NULL, and for a pointer that is none of the case's. */
#define SPFB_NULL 0xfe
#define SPFB_OTHER 0xff

static const char *const spfb_names[SPFB_NODES] = {"filter", "parent", "below"};

/* One call on node `filter` (-1: NULL) with `format`, `space` and
 * `allow_direct`; `plain` for obs_source_process_filter_begin() (no
 * space). The lookups answer node `target` and node `parent` (-1: NULL);
 * the base size answers `width` and `height`, gs_get_color_space()
 * `current`. Each node has its output flags, bypass flag and a texrender
 * or none; gs_texrender_get_format() answers `texrender_format`,
 * gs_texrender_create() a new texrender when `creates`,
 * gs_texrender_begin_with_color_space() `begins` for a texrender. */
struct spfb_case {
	int8_t filter;
	int8_t target;
	int8_t parent;
	uint8_t noise;
	bool plain;
	bool creates;
	bool begins;
	bool has_name;
	int32_t format;
	int32_t space;
	int32_t allow_direct;
	int32_t current;
	int32_t texrender_format;
	uint32_t width;
	uint32_t height;
	uint32_t flags[SPFB_NODES];
	bool bypass[SPFB_NODES];
	bool texrender[SPFB_NODES];
};

/* What one side did: its result, each call in order as {kind, a, b, c, d},
 * its log lines (count, level of the last, the last), each node's bypass
 * flag, allow_direct and texrender after, and whether every other byte of
 * the nodes is as before. */
struct spfb_result {
	bool result;
	uint32_t events;
	int64_t trace[SPFB_EVENTS][5];
	uint32_t log_count;
	int32_t log_level;
	char log[128];
	bool bypass[SPFB_NODES];
	int32_t allow_direct[SPFB_NODES];
	int64_t texrender[SPFB_NODES];
	bool untouched;
};

struct spfb_fixture {
	struct obs_source nodes[SPFB_NODES];
	char texrenders[SPFB_TEXRENDERS];
	char names[SPFB_NODES][8];
	const struct spfb_case *c;
	struct spfb_result *out;
};

/* Per thread: the Rust test harness runs cases on several threads. */
static _Thread_local struct spfb_fixture *spfb_current;

static int64_t spfb_node_index(const void *source)
{
	struct spfb_fixture *f = spfb_current;
	if (!source)
		return SPFB_NULL;
	for (size_t i = 0; i < SPFB_NODES; i++) {
		if (source == &f->nodes[i])
			return (int64_t)i;
	}
	return SPFB_OTHER;
}

static int64_t spfb_texrender_index(const void *texrender)
{
	struct spfb_fixture *f = spfb_current;
	if (!texrender)
		return SPFB_NULL;
	for (size_t i = 0; i < SPFB_TEXRENDERS; i++) {
		if (texrender == &f->texrenders[i])
			return (int64_t)i;
	}
	return SPFB_OTHER;
}

static obs_source_t *spfb_node(int8_t index)
{
	return index < 0 || index >= SPFB_NODES ? NULL : &spfb_current->nodes[index];
}

static void spfb_event(int64_t kind, int64_t a, int64_t b, int64_t c, int64_t d)
{
	struct spfb_result *out = spfb_current->out;
	if (out->events < SPFB_EVENTS) {
		out->trace[out->events][0] = kind;
		out->trace[out->events][1] = a;
		out->trace[out->events][2] = b;
		out->trace[out->events][3] = c;
		out->trace[out->events][4] = d;
	}
	out->events++;
}

/* A float argument as its bits, so -0.0 and NaN compare exactly. */
static int64_t spfb_bits(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return bits;
}

/* ------------------------------------------------------------------------- */
/* the stand-ins */

obs_source_t *spfb_get_target(const obs_source_t *filter);
obs_source_t *spfb_get_target(const obs_source_t *filter)
{
	/* What the lookup would see of the bypass flag: begin clears it first. */
	spfb_event('T', spfb_node_index(filter), filter ? filter->filter_bypass_active : -1, 0, 0);
	return spfb_node(spfb_current->c->target);
}

obs_source_t *spfb_get_parent(const obs_source_t *filter);
obs_source_t *spfb_get_parent(const obs_source_t *filter)
{
	spfb_event('P', spfb_node_index(filter), 0, 0, 0);
	return spfb_node(spfb_current->c->parent);
}

uint32_t spfb_base_width(obs_source_t *source);
uint32_t spfb_base_width(obs_source_t *source)
{
	spfb_event('W', spfb_node_index(source), 0, 0, 0);
	return spfb_current->c->width;
}

uint32_t spfb_base_height(obs_source_t *source);
uint32_t spfb_base_height(obs_source_t *source)
{
	spfb_event('H', spfb_node_index(source), 0, 0, 0);
	return spfb_current->c->height;
}

enum gs_color_space spfb_get_color_space(void);
enum gs_color_space spfb_get_color_space(void)
{
	spfb_event('C', 0, 0, 0, 0);
	return (enum gs_color_space)spfb_current->c->current;
}

void spfb_skip_video_filter(obs_source_t *filter);
void spfb_skip_video_filter(obs_source_t *filter)
{
	spfb_event('S', spfb_node_index(filter), 0, 0, 0);
}

enum gs_color_format spfb_texrender_get_format(const gs_texrender_t *texrender);
enum gs_color_format spfb_texrender_get_format(const gs_texrender_t *texrender)
{
	spfb_event('F', spfb_texrender_index(texrender), 0, 0, 0);
	return (enum gs_color_format)spfb_current->c->texrender_format;
}

void spfb_texrender_destroy(gs_texrender_t *texrender);
void spfb_texrender_destroy(gs_texrender_t *texrender)
{
	spfb_event('D', spfb_texrender_index(texrender), 0, 0, 0);
}

gs_texrender_t *spfb_texrender_create(enum gs_color_format format, enum gs_zstencil_format zsformat);
gs_texrender_t *spfb_texrender_create(enum gs_color_format format, enum gs_zstencil_format zsformat)
{
	spfb_event('N', format, zsformat, 0, 0);
	return spfb_current->c->creates ? (gs_texrender_t *)&spfb_current->texrenders[SPFB_NODES] : NULL;
}

bool spfb_texrender_begin_with_color_space(gs_texrender_t *texrender, uint32_t cx, uint32_t cy,
					   enum gs_color_space space);
bool spfb_texrender_begin_with_color_space(gs_texrender_t *texrender, uint32_t cx, uint32_t cy,
					   enum gs_color_space space)
{
	spfb_event('G', spfb_texrender_index(texrender), cx, cy, space);
	return texrender && spfb_current->c->begins;
}

void spfb_texrender_end(gs_texrender_t *texrender);
void spfb_texrender_end(gs_texrender_t *texrender)
{
	spfb_event('Z', spfb_texrender_index(texrender), 0, 0, 0);
}

void spfb_blend_state_push(void);
void spfb_blend_state_push(void)
{
	spfb_event('U', 0, 0, 0, 0);
}

void spfb_blend_state_pop(void);
void spfb_blend_state_pop(void)
{
	spfb_event('Q', 0, 0, 0, 0);
}

void spfb_blend_function_separate(enum gs_blend_type src_c, enum gs_blend_type dest_c, enum gs_blend_type src_a,
				  enum gs_blend_type dest_a);
void spfb_blend_function_separate(enum gs_blend_type src_c, enum gs_blend_type dest_c, enum gs_blend_type src_a,
				  enum gs_blend_type dest_a)
{
	spfb_event('J', src_c, dest_c, src_a, dest_a);
}

void spfb_clear(uint32_t clear_flags, const struct vec4 *color, float depth, uint8_t stencil);
void spfb_clear(uint32_t clear_flags, const struct vec4 *color, float depth, uint8_t stencil)
{
	const bool zero = color && color->x == 0.0f && color->y == 0.0f && color->z == 0.0f && color->w == 0.0f;
	spfb_event('K', clear_flags, zero, spfb_bits(depth), stencil);
}

void spfb_ortho(float left, float right, float top, float bottom, float znear, float zfar);
void spfb_ortho(float left, float right, float top, float bottom, float znear, float zfar)
{
	spfb_event('O', spfb_bits(right), spfb_bits(bottom),
		   (int64_t)((uint64_t)spfb_bits(left) << 32 | (uint64_t)spfb_bits(top)),
		   (int64_t)((uint64_t)spfb_bits(znear) << 32 | (uint64_t)spfb_bits(zfar)));
}

void spfb_default_render(obs_source_t *source);
void spfb_default_render(obs_source_t *source)
{
	spfb_event('E', spfb_node_index(source), 0, 0, 0);
}

void spfb_video_render(obs_source_t *source);
void spfb_video_render(obs_source_t *source)
{
	spfb_event('V', spfb_node_index(source), 0, 0, 0);
}

/* ------------------------------------------------------------------------- */
/* the original and the production adapters, over the stand-ins */

#define obs_filter_get_target spfb_get_target
#define obs_filter_get_parent spfb_get_parent
#define obs_source_base_width spfb_base_width
#define obs_source_base_height spfb_base_height
#define gs_get_color_space spfb_get_color_space
#define obs_source_skip_video_filter spfb_skip_video_filter
#define gs_texrender_get_format spfb_texrender_get_format
#define gs_texrender_destroy spfb_texrender_destroy
#define gs_texrender_create spfb_texrender_create
#define gs_texrender_begin_with_color_space spfb_texrender_begin_with_color_space
#define gs_texrender_end spfb_texrender_end
#define gs_blend_state_push spfb_blend_state_push
#define gs_blend_state_pop spfb_blend_state_pop
#define gs_blend_function_separate spfb_blend_function_separate
#define gs_clear spfb_clear
#define gs_ortho spfb_ortho
#define obs_source_default_render spfb_default_render
#define obs_source_video_render spfb_video_render
#define obs_source_process_filter_begin oracle_obs_source_process_filter_begin
#define obs_source_process_filter_begin_with_color_space oracle_obs_source_process_filter_begin_with_color_space
bool oracle_obs_source_process_filter_begin(obs_source_t *filter, enum gs_color_format format,
					    enum obs_allow_direct_render allow_direct);
bool oracle_obs_source_process_filter_begin_with_color_space(obs_source_t *filter, enum gs_color_format format,
							     enum gs_color_space space,
							     enum obs_allow_direct_render allow_direct);
#include "source-process-filter-begin.inc"
#undef obs_source_process_filter_begin_with_color_space
#undef obs_source_process_filter_begin
#include "source_process_filter_begin_fields.inc"
#undef obs_source_video_render
#undef obs_source_default_render
#undef gs_ortho
#undef gs_clear
#undef gs_blend_function_separate
#undef gs_blend_state_pop
#undef gs_blend_state_push
#undef gs_texrender_end
#undef gs_texrender_begin_with_color_space
#undef gs_texrender_create
#undef gs_texrender_destroy
#undef gs_texrender_get_format
#undef obs_source_skip_video_filter
#undef gs_get_color_space
#undef obs_source_base_height
#undef obs_source_base_width
#undef obs_filter_get_parent
#undef obs_filter_get_target

/* ------------------------------------------------------------------------- */
/* the parity run */

#if defined(__GNUC__) || defined(__clang__)
static void spfb_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void spfb_capture(int level, const char *format, va_list args, void *param)
{
	struct spfb_result *out = param;
	out->log_count++;
	out->log_level = level;
	vsnprintf(out->log, sizeof(out->log), format, args);
}

/* The C ABI of both: enums are passed as int. */
typedef bool (*spfb_begin)(obs_source_t *, int, int);
typedef bool (*spfb_begin_space)(obs_source_t *, int, int, int);
typedef void (*spfb_get_log)(log_handler_t *, void **);
typedef void (*spfb_set_log)(log_handler_t, void *);

_Static_assert(sizeof(enum gs_color_format) == sizeof(int), "enum gs_color_format is passed as int");
_Static_assert(sizeof(enum gs_color_space) == sizeof(int), "enum gs_color_space is passed as int");
_Static_assert(sizeof(enum obs_allow_direct_render) == sizeof(int), "enum obs_allow_direct_render is passed as int");

/* The originals behind the C ABI both have. */
static bool spfb_original_begin(obs_source_t *filter, int format, int allow_direct)
{
	return oracle_obs_source_process_filter_begin(filter, (enum gs_color_format)format,
						      (enum obs_allow_direct_render)allow_direct);
}

static bool spfb_original_begin_space(obs_source_t *filter, int format, int space, int allow_direct)
{
	return oracle_obs_source_process_filter_begin_with_color_space(filter, (enum gs_color_format)format,
								       (enum gs_color_space)space,
								       (enum obs_allow_direct_render)allow_direct);
}

/* The originals, for the parity test's other side. */
spfb_begin oracle_source_process_filter_begin_original(void);
spfb_begin oracle_source_process_filter_begin_original(void)
{
	return spfb_original_begin;
}

spfb_begin_space oracle_source_process_filter_begin_with_color_space_original(void);
spfb_begin_space oracle_source_process_filter_begin_with_color_space_original(void)
{
	return spfb_original_begin_space;
}

/* Runs one case through `begin` (c->plain) or `begin_space` on fresh
 * noise-filled nodes. False when the fixture could not be built or an
 * adapter does not reach its field. */
bool oracle_source_process_filter_begin_run(spfb_begin begin, spfb_begin_space begin_space, spfb_get_log get,
					    spfb_set_log set, const struct spfb_case *c, struct spfb_result *out);
bool oracle_source_process_filter_begin_run(spfb_begin begin, spfb_begin_space begin_space, spfb_get_log get,
					    spfb_set_log set, const struct spfb_case *c, struct spfb_result *out)
{
	struct spfb_fixture *f = malloc(sizeof(*f));
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
	for (size_t i = 0; i < SPFB_NODES; i++) {
		f->nodes[i].info.output_flags = c->flags[i];
		f->nodes[i].filter_bypass_active = c->bypass[i];
		f->nodes[i].filter_texrender = c->texrender[i] ? (gs_texrender_t *)&f->texrenders[i] : NULL;
		snprintf(f->names[i], sizeof(f->names[i]), "%s", spfb_names[i]);
		f->nodes[i].context.name = c->has_name ? f->names[i] : NULL;
	}
	spfb_current = f;

	/* The real adapters reach the fields they name. */
	bool valid = true;
	for (size_t i = 0; i < SPFB_NODES; i++) {
		valid = valid && obs_source_process_filter_begin_flags(&f->nodes[i]) == c->flags[i] &&
			obs_source_process_filter_begin_has_texrender(&f->nodes[i]) == c->texrender[i] &&
			obs_source_process_filter_begin_name(&f->nodes[i]) == f->nodes[i].context.name;
	}
	memcpy(before, f->nodes, sizeof(f->nodes));

	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	set(spfb_capture, out);
	if (c->plain)
		out->result = begin(spfb_node(c->filter), c->format, c->allow_direct);
	else
		out->result = begin_space(spfb_node(c->filter), c->format, c->space, c->allow_direct);
	set(old_handler, old_param);
	/* The bypass flags, allow_direct and texrenders are reported; every
	 * other byte must be as before. */
	for (size_t i = 0; i < SPFB_NODES; i++) {
		out->bypass[i] = f->nodes[i].filter_bypass_active;
		out->allow_direct[i] = (int32_t)f->nodes[i].allow_direct;
		out->texrender[i] = spfb_texrender_index(f->nodes[i].filter_texrender);
		before[i].filter_bypass_active = f->nodes[i].filter_bypass_active;
		before[i].allow_direct = f->nodes[i].allow_direct;
		before[i].filter_texrender = f->nodes[i].filter_texrender;
	}
	spfb_current = NULL;
	out->untouched = memcmp(before, f->nodes, sizeof(f->nodes)) == 0;
	free(before);
	free(f);
	return valid;
}

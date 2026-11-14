/* Tier 3 for obs-graphics::matrix_entrypoints: the original matrix calls
 * (matrix-entrypoints.c.h, renamed oracle_gs_matrix_*) over the original C
 * matrix stack (oracle/graphics_state_stacks.c, oracle_gs_matrix_stack_*),
 * and the production adapter (matrix-entrypoints-fields.c.h, under its real
 * names, which the Rust shims call), over the real private graphics layout,
 * a thread-local stand-in context per side and a recording blog.
 *
 * Side 0 is the C, side 1 the Rust: each keeps its own context and matrix
 * stack, so a sequence runs on both and the stacks are compared after every
 * call.
 *
 * Outside these tests (no oracle_matrix_init on the thread) there is no
 * context, and the debug line of gs_matrix_push, _pop and _identity goes to
 * the texrender device's call log as "mat_push", "mat_pop" and
 * "mat_identity" (oracle/texture_render_device.c), as its stand-ins under
 * the real names did before the Rust calls replaced them. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_gs_matrix_push(void);
void oracle_gs_matrix_pop(void);
void oracle_gs_matrix_identity(void);
void oracle_gs_matrix_transpose(void);
void oracle_gs_matrix_set(const struct matrix4 *matrix);
void oracle_gs_matrix_get(struct matrix4 *dst);
void oracle_gs_matrix_mul(const struct matrix4 *matrix);
void oracle_gs_matrix_rotquat(const struct quat *rot);
void oracle_gs_matrix_rotaa(const struct axisang *rot);
void oracle_gs_matrix_translate(const struct vec3 *pos);
void oracle_gs_matrix_scale(const struct vec3 *scale);
void oracle_gs_matrix_rotaa4f(float x, float y, float z, float angle);
void oracle_gs_matrix_translate3f(float x, float y, float z);
void oracle_gs_matrix_scale3f(float x, float y, float z);

/* The original stack (oracle/graphics_state_stacks.c). */
void oracle_gs_matrix_stack_push(struct darray *stack, size_t *cur);
bool oracle_gs_matrix_stack_pop(struct darray *stack, size_t *cur);
void oracle_gs_matrix_stack_identity(struct darray *stack, size_t cur);
void oracle_gs_matrix_stack_transpose(struct darray *stack, size_t cur);
void oracle_gs_matrix_stack_set(struct darray *stack, size_t cur, const struct matrix4 *matrix);
void oracle_gs_matrix_stack_get(const struct darray *stack, size_t cur, struct matrix4 *dst);
void oracle_gs_matrix_stack_mul(struct darray *stack, size_t cur, const struct matrix4 *matrix);
void oracle_gs_matrix_stack_rotquat(struct darray *stack, size_t cur, const struct quat *rot);
void oracle_gs_matrix_stack_rotaa(struct darray *stack, size_t cur, const struct axisang *rot);
void oracle_gs_matrix_stack_translate(struct darray *stack, size_t cur, const struct vec3 *pos);
void oracle_gs_matrix_stack_scale(struct darray *stack, size_t cur, const struct vec3 *scale);

void oracle_texrender_device_note(const char *text);

void oracle_matrix_init(void);
void oracle_matrix_free(void);
void oracle_matrix_select(int side, bool context);
size_t oracle_matrix_read(int side, struct matrix4 *out, size_t max, size_t *cur);
unsigned oracle_matrix_logs(void);
int oracle_matrix_level(void);
const char *oracle_matrix_log(void);

static THREAD_LOCAL struct graphics_subsystem me_graphics[2];
static THREAD_LOCAL bool me_active;
static THREAD_LOCAL int me_side;
static THREAD_LOCAL bool me_has_context;
static THREAD_LOCAL unsigned me_logs;
static THREAD_LOCAL int me_level;
static THREAD_LOCAL char me_log[160];

static graphics_t *me_context(void)
{
	return me_active && me_has_context ? &me_graphics[me_side] : NULL;
}

/* "gs_matrix_<name>: ..." as the texrender device's "mat_<name>". */
static void me_texrender_note(const char *line)
{
	static const char prefix[] = "gs_matrix_";
	const char *colon = strchr(line, ':');
	char note[64];

	if (!colon || strncmp(line, prefix, sizeof(prefix) - 1) != 0)
		return;
	snprintf(note, sizeof(note), "mat_%.*s", (int)(colon - line - (ptrdiff_t)(sizeof(prefix) - 1)),
		 line + sizeof(prefix) - 1);
	oracle_texrender_device_note(note);
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void me_blog(int level, const char *format, ...)
{
	char line[sizeof(me_log)];
	va_list args;
	va_start(args, format);
	vsnprintf(line, sizeof(line), format, args);
	va_end(args);

	if (!me_active) {
		me_texrender_note(line);
		return;
	}
	me_logs++;
	me_level = level;
	memcpy(me_log, line, sizeof(me_log));
}

#define gs_get_context me_context
#define blog me_blog
#include "matrix-entrypoints-fields.inc"
#define gs_matrix_push oracle_gs_matrix_push
#define gs_matrix_pop oracle_gs_matrix_pop
#define gs_matrix_identity oracle_gs_matrix_identity
#define gs_matrix_transpose oracle_gs_matrix_transpose
#define gs_matrix_set oracle_gs_matrix_set
#define gs_matrix_get oracle_gs_matrix_get
#define gs_matrix_mul oracle_gs_matrix_mul
#define gs_matrix_rotquat oracle_gs_matrix_rotquat
#define gs_matrix_rotaa oracle_gs_matrix_rotaa
#define gs_matrix_translate oracle_gs_matrix_translate
#define gs_matrix_scale oracle_gs_matrix_scale
#define gs_matrix_rotaa4f oracle_gs_matrix_rotaa4f
#define gs_matrix_translate3f oracle_gs_matrix_translate3f
#define gs_matrix_scale3f oracle_gs_matrix_scale3f
#define gs_matrix_stack_push oracle_gs_matrix_stack_push
#define gs_matrix_stack_pop oracle_gs_matrix_stack_pop
#define gs_matrix_stack_identity oracle_gs_matrix_stack_identity
#define gs_matrix_stack_transpose oracle_gs_matrix_stack_transpose
#define gs_matrix_stack_set oracle_gs_matrix_stack_set
#define gs_matrix_stack_get oracle_gs_matrix_stack_get
#define gs_matrix_stack_mul oracle_gs_matrix_stack_mul
#define gs_matrix_stack_rotquat oracle_gs_matrix_stack_rotquat
#define gs_matrix_stack_rotaa oracle_gs_matrix_stack_rotaa
#define gs_matrix_stack_translate oracle_gs_matrix_stack_translate
#define gs_matrix_stack_scale oracle_gs_matrix_stack_scale
#include "matrix-entrypoints.inc"
#undef blog
#undef gs_get_context

/* Both sides as graphics_init leaves them: one identity matrix, top 0. */
void oracle_matrix_init(void)
{
	oracle_matrix_free();
	for (int side = 0; side < 2; side++) {
		struct matrix4 id;
		vec4_set(&id.x, 1.0f, 0.0f, 0.0f, 0.0f);
		vec4_set(&id.y, 0.0f, 1.0f, 0.0f, 0.0f);
		vec4_set(&id.z, 0.0f, 0.0f, 1.0f, 0.0f);
		vec4_set(&id.t, 0.0f, 0.0f, 0.0f, 1.0f);
		da_push_back(me_graphics[side].matrix_stack, &id);
		me_graphics[side].cur_matrix = 0;
	}
	me_active = true;
}

void oracle_matrix_free(void)
{
	for (int side = 0; side < 2; side++) {
		da_free(me_graphics[side].matrix_stack);
		memset(&me_graphics[side], 0, sizeof(me_graphics[side]));
	}
	me_active = false;
}

/* The next call runs on `side`, with or without a context; the log is
 * cleared. */
void oracle_matrix_select(int side, bool context)
{
	me_side = side;
	me_has_context = context;
	me_logs = 0;
	me_level = 0;
	me_log[0] = '\0';
}

size_t oracle_matrix_read(int side, struct matrix4 *out, size_t max, size_t *cur)
{
	const struct graphics_subsystem *g = &me_graphics[side];
	size_t n = g->matrix_stack.num < max ? g->matrix_stack.num : max;
	memcpy(out, g->matrix_stack.array, n * sizeof(*out));
	*cur = g->cur_matrix;
	return g->matrix_stack.num;
}

unsigned oracle_matrix_logs(void)
{
	return me_logs;
}

int oracle_matrix_level(void)
{
	return me_level;
}

const char *oracle_matrix_log(void)
{
	return me_log;
}

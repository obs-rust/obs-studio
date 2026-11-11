/* Original extracted C orchestration, real private layout, instrumented device calls. */
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "graphics/graphics-state-stacks.h"
#include "util/base.h"

static THREAD_LOCAL struct graphics_subsystem matrix_graphics;
static THREAD_LOCAL bool matrix_present;
static THREAD_LOCAL unsigned matrix_logs;
static THREAD_LOCAL char matrix_message[128];

static graphics_t *matrix_context(void)
{
	return matrix_present ? &matrix_graphics : NULL;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void matrix_blog(int level, const char *format, ...)
{
	(void)level;
	va_list args;
	va_start(args, format);
	matrix_logs++;
	vsnprintf(matrix_message, sizeof(matrix_message), format, args);
	va_end(args);
}
#define gs_get_context matrix_context
#define blog matrix_blog
#include "matrix-entrypoints-fields.inc"
#undef gs_get_context

/* These are device mocks with the same state side effects as the public setters. */
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
#define thread_graphics matrix_context()
#define gs_valid(function) (obs_graphics_matrix_context(function) != NULL)
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
#include "state-matrix-entrypoints.inc"
#undef gs_matrix_scale3f
#undef gs_matrix_translate3f
#undef gs_matrix_rotaa4f
#undef gs_matrix_scale
#undef gs_matrix_translate
#undef gs_matrix_rotaa
#undef gs_matrix_rotquat
#undef gs_matrix_mul
#undef gs_matrix_get
#undef gs_matrix_set
#undef gs_matrix_transpose
#undef gs_matrix_identity
#undef gs_matrix_pop
#undef gs_matrix_push
#undef gs_matrix_stack_scale
#undef gs_matrix_stack_translate
#undef gs_matrix_stack_rotaa
#undef gs_matrix_stack_rotquat
#undef gs_matrix_stack_mul
#undef gs_matrix_stack_get
#undef gs_matrix_stack_set
#undef gs_matrix_stack_transpose
#undef gs_matrix_stack_identity
#undef gs_matrix_stack_pop
#undef gs_matrix_stack_push
#undef gs_valid
#undef thread_graphics
#undef blog

void oracle_matrix_begin(bool present, const struct matrix4 *initial);
void oracle_matrix_set(bool present);
void oracle_matrix_end(void);
unsigned oracle_matrix_logs(void);
const char *oracle_matrix_message(void);
size_t oracle_matrix_depth(void);
struct matrix4 oracle_matrix_current(void);
struct matrix4 oracle_matrix_saved(size_t index);

void oracle_matrix_end(void)
{
	da_free(matrix_graphics.matrix_stack);
}
void oracle_matrix_set(bool present)
{
	matrix_present = present;
}
void oracle_matrix_begin(bool present, const struct matrix4 *initial)
{
	oracle_matrix_end();
	memset(&matrix_graphics, 0, sizeof(matrix_graphics));
	matrix_logs = 0;
	matrix_message[0] = '\0';
	/* Seed the stack with one matrix on the heap: end() frees it. */
	matrix_graphics.matrix_stack.array = bmalloc(sizeof(struct matrix4) * 4);
	matrix_graphics.matrix_stack.num = 1;
	matrix_graphics.matrix_stack.capacity = 4;
	if (initial != NULL)
		*matrix_graphics.matrix_stack.array = *initial;
	oracle_matrix_set(present);
}
unsigned oracle_matrix_logs(void)
{
	return matrix_logs;
}
const char *oracle_matrix_message(void)
{
	return matrix_message;
}
size_t oracle_matrix_depth(void)
{
	return matrix_graphics.matrix_stack.da.num;
}
struct matrix4 oracle_matrix_current(void)
{
	return matrix_graphics.matrix_stack.array[matrix_graphics.cur_matrix];
}
struct matrix4 oracle_matrix_saved(size_t index)
{
	return matrix_graphics.matrix_stack.array[index];
}

/* Original extracted C orchestration, real private layout, instrumented device calls. */
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "graphics/graphics-state-stacks.h"
#include "util/base.h"

static THREAD_LOCAL struct graphics_subsystem blend_graphics;
static THREAD_LOCAL bool blend_present;
static THREAD_LOCAL unsigned blend_calls[3], blend_count, blend_logs;
static THREAD_LOCAL char blend_message[128];

static graphics_t *blend_context(void)
{
	return blend_present ? &blend_graphics : NULL;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void blend_blog(int level, const char *format, ...)
{
	(void)level;
	va_list args;
	va_start(args, format);
	blend_logs++;
	vsnprintf(blend_message, sizeof(blend_message), format, args);
	va_end(args);
}
#define gs_get_context blend_context
#define blog blend_blog
#include "blend-entrypoints-fields.inc"
#undef blog
#undef gs_get_context

/* These are device mocks with the same state side effects as the public setters. */
void gs_enable_blending(bool value)
{
	blend_graphics.cur_blend_state.enabled = value;
	blend_calls[blend_count++] = 1;
}
void gs_blend_function_separate(enum gs_blend_type sc, enum gs_blend_type dc, enum gs_blend_type sa,
				enum gs_blend_type da)
{
	struct blend_state *s = &blend_graphics.cur_blend_state;
	s->src_c = sc;
	s->dest_c = dc;
	s->src_a = sa;
	s->dest_a = da;
	blend_calls[blend_count++] = 2;
}
void gs_blend_op(enum gs_blend_op_type value)
{
	blend_graphics.cur_blend_state.op = value;
	blend_calls[blend_count++] = 3;
}
/* All stack/reset helpers on the oracle side are the original C as well. */
void oracle_gs_blend_state_stack_push(struct darray *, const struct blend_state *);
bool oracle_gs_blend_state_stack_pop(struct darray *, struct blend_state *);
struct gs_blend_reset oracle_gs_blend_state_reset_steps(const struct blend_state *);
void oracle_gs_blend_state_push(void);
void oracle_gs_blend_state_pop(void);
void oracle_gs_reset_blend_state(void);
#define thread_graphics blend_context()
#define gs_valid(function) (obs_graphics_blend_context(function) != NULL)
#define gs_blend_state_stack_push oracle_gs_blend_state_stack_push
#define gs_blend_state_stack_pop oracle_gs_blend_state_stack_pop
#define gs_blend_state_reset_steps oracle_gs_blend_state_reset_steps
#define gs_blend_state_push oracle_gs_blend_state_push
#define gs_blend_state_pop oracle_gs_blend_state_pop
#define gs_reset_blend_state oracle_gs_reset_blend_state
#include "state-blend-entrypoints.inc"
#undef gs_reset_blend_state
#undef gs_blend_state_pop
#undef gs_blend_state_push
#undef gs_blend_state_reset_steps
#undef gs_blend_state_stack_pop
#undef gs_blend_state_stack_push
#undef gs_valid
#undef thread_graphics

void oracle_blend_begin(bool present, const struct blend_state *state);
void oracle_blend_set(bool present, const struct blend_state *state);
void oracle_blend_clear_calls(void);
void oracle_blend_end(void);
unsigned oracle_blend_count(void);
unsigned oracle_blend_call(unsigned index);
unsigned oracle_blend_logs(void);
const char *oracle_blend_message(void);
size_t oracle_blend_depth(void);
struct blend_state oracle_blend_current(void);
struct blend_state oracle_blend_saved(size_t index);

void oracle_blend_end(void)
{
	da_free(blend_graphics.blend_state_stack);
}
void oracle_blend_set(bool present, const struct blend_state *state)
{
	blend_present = present;
	blend_graphics.cur_blend_state = *state;
}
void oracle_blend_begin(bool present, const struct blend_state *state)
{
	oracle_blend_end();
	memset(&blend_graphics, 0, sizeof(blend_graphics));
	blend_logs = blend_count = 0;
	blend_message[0] = '\0';
	oracle_blend_set(present, state);
}
void oracle_blend_clear_calls(void)
{
	blend_count = 0;
}
unsigned oracle_blend_count(void)
{
	return blend_count;
}
unsigned oracle_blend_call(unsigned index)
{
	return blend_calls[index];
}
unsigned oracle_blend_logs(void)
{
	return blend_logs;
}
const char *oracle_blend_message(void)
{
	return blend_message;
}
size_t oracle_blend_depth(void)
{
	return blend_graphics.blend_state_stack.num;
}
struct blend_state oracle_blend_current(void)
{
	return blend_graphics.cur_blend_state;
}
struct blend_state oracle_blend_saved(size_t index)
{
	return blend_graphics.blend_state_stack.array[index];
}

size_t oracle_blend_layout(unsigned index);
size_t oracle_blend_layout(unsigned index)
{
	const size_t values[] = {sizeof(struct blend_state),
				 _Alignof(struct blend_state),
				 offsetof(struct blend_state, enabled),
				 offsetof(struct blend_state, src_c),
				 offsetof(struct blend_state, dest_c),
				 offsetof(struct blend_state, src_a),
				 offsetof(struct blend_state, dest_a),
				 offsetof(struct blend_state, op)};
	return values[index];
}

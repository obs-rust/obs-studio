/* Tier 3 for obs-graphics::blend_entrypoints: the original setters
 * (state-blend-setters.c.h) and push, pop and reset
 * (state-blend-entrypoints.c.h), renamed oracle_gs_*, and the production
 * adapter (blend-entrypoints-fields.c.h, under its real names, which the Rust
 * shims call), over the real private layout, a thread-local stand-in
 * context, a recording device and a recording blog. */
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "graphics/graphics-state-stacks.h"
#include "util/base.h"

/* One device call: `call` is 1 enable, 2 separate factors, 3 operation,
 * 4 one factor pair; `seen` is the blend state when the device was called. */
struct oracle_blend_event {
	unsigned call;
	const void *device;
	uint32_t args[4];
	struct blend_state seen;
};

#define BLEND_MAX_EVENTS 4

static THREAD_LOCAL struct graphics_subsystem blend_graphics;
static THREAD_LOCAL bool blend_present;
static THREAD_LOCAL struct oracle_blend_event blend_events[BLEND_MAX_EVENTS];
static THREAD_LOCAL unsigned blend_count, blend_logs;
static THREAD_LOCAL int blend_level;
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
	va_list args;
	va_start(args, format);
	blend_logs++;
	blend_level = level;
	vsnprintf(blend_message, sizeof(blend_message), format, args);
	va_end(args);
}
#define gs_get_context blend_context
#define blog blend_blog
#include "blend-entrypoints-fields.inc"
#undef blog
#undef gs_get_context

static struct oracle_blend_event *blend_record(unsigned call, const gs_device_t *device)
{
	/* An overflow is a test bug: pop, the longest, makes three calls. */
	if (blend_count >= BLEND_MAX_EVENTS)
		abort();
	struct oracle_blend_event *event = &blend_events[blend_count++];
	memset(event, 0, sizeof(*event));
	event->call = call;
	event->device = device;
	event->seen = blend_graphics.cur_blend_state;
	return event;
}
static void blend_device_enable(gs_device_t *device, bool enable)
{
	blend_record(1, device)->args[0] = enable;
}
static void blend_device_separate(gs_device_t *device, enum gs_blend_type sc, enum gs_blend_type dc,
				  enum gs_blend_type sa, enum gs_blend_type da)
{
	struct oracle_blend_event *event = blend_record(2, device);
	event->args[0] = (uint32_t)sc;
	event->args[1] = (uint32_t)dc;
	event->args[2] = (uint32_t)sa;
	event->args[3] = (uint32_t)da;
}
static void blend_device_op(gs_device_t *device, enum gs_blend_op_type op)
{
	blend_record(3, device)->args[0] = (uint32_t)op;
}
static void blend_device_function(gs_device_t *device, enum gs_blend_type src, enum gs_blend_type dest)
{
	struct oracle_blend_event *event = blend_record(4, device);
	event->args[0] = (uint32_t)src;
	event->args[1] = (uint32_t)dest;
}

/* All stack/reset helpers on the oracle side are the original C as well. */
void oracle_gs_blend_state_stack_push(struct darray *, const struct blend_state *);
bool oracle_gs_blend_state_stack_pop(struct darray *, struct blend_state *);
struct gs_blend_reset oracle_gs_blend_state_reset_steps(const struct blend_state *);
void oracle_gs_enable_blending(bool enable);
void oracle_gs_blend_function(enum gs_blend_type src, enum gs_blend_type dest);
void oracle_gs_blend_function_separate(enum gs_blend_type src_c, enum gs_blend_type dest_c, enum gs_blend_type src_a,
				       enum gs_blend_type dest_a);
void oracle_gs_blend_op(enum gs_blend_op_type op);
void oracle_gs_blend_state_push(void);
void oracle_gs_blend_state_pop(void);
void oracle_gs_reset_blend_state(void);
#define thread_graphics blend_context()
#define gs_valid(function) (obs_graphics_blend_context(function) != NULL)
#define gs_blend_state_stack_push oracle_gs_blend_state_stack_push
#define gs_blend_state_stack_pop oracle_gs_blend_state_stack_pop
#define gs_blend_state_reset_steps oracle_gs_blend_state_reset_steps
#define gs_enable_blending oracle_gs_enable_blending
#define gs_blend_function oracle_gs_blend_function
#define gs_blend_function_separate oracle_gs_blend_function_separate
#define gs_blend_op oracle_gs_blend_op
#define gs_blend_state_push oracle_gs_blend_state_push
#define gs_blend_state_pop oracle_gs_blend_state_pop
#define gs_reset_blend_state oracle_gs_reset_blend_state
#include "state-blend-setters.inc"
#include "state-blend-entrypoints.inc"
#undef gs_reset_blend_state
#undef gs_blend_state_pop
#undef gs_blend_state_push
#undef gs_blend_op
#undef gs_blend_function_separate
#undef gs_blend_function
#undef gs_enable_blending
#undef gs_blend_state_reset_steps
#undef gs_blend_state_stack_pop
#undef gs_blend_state_stack_push
#undef gs_valid
#undef thread_graphics

void oracle_blend_begin(bool present, const struct blend_state *state);
void oracle_blend_set(bool present, const struct blend_state *state);
void oracle_blend_device(void *device);
void oracle_blend_clear_calls(void);
void oracle_blend_end(void);
unsigned oracle_blend_count(void);
unsigned oracle_blend_call(unsigned index);
struct oracle_blend_event oracle_blend_event(unsigned index);
unsigned oracle_blend_logs(void);
int oracle_blend_level(void);
const char *oracle_blend_message(void);
size_t oracle_blend_depth(void);
struct blend_state oracle_blend_current(void);
struct blend_state oracle_blend_saved(size_t index);

static unsigned char blend_devices[2];

void oracle_blend_end(void)
{
	da_free(blend_graphics.blend_state_stack);
}
void oracle_blend_set(bool present, const struct blend_state *state)
{
	blend_present = present;
	blend_graphics.cur_blend_state = *state;
}
void oracle_blend_device(void *device)
{
	blend_graphics.device = (gs_device_t *)device;
}
void oracle_blend_begin(bool present, const struct blend_state *state)
{
	oracle_blend_end();
	memset(&blend_graphics, 0, sizeof(blend_graphics));
	blend_graphics.device = (gs_device_t *)&blend_devices[0];
	blend_graphics.exports.device_enable_blending = blend_device_enable;
	blend_graphics.exports.device_blend_function = blend_device_function;
	blend_graphics.exports.device_blend_function_separate = blend_device_separate;
	blend_graphics.exports.device_blend_op = blend_device_op;
	blend_logs = blend_count = 0;
	blend_level = 0;
	blend_message[0] = '\0';
	oracle_blend_set(present, state);
}
void oracle_blend_clear_calls(void)
{
	blend_count = 0;
	blend_logs = 0;
	blend_level = 0;
	blend_message[0] = '\0';
}
unsigned oracle_blend_count(void)
{
	return blend_count;
}
unsigned oracle_blend_call(unsigned index)
{
	return blend_events[index].call;
}
struct oracle_blend_event oracle_blend_event(unsigned index)
{
	return blend_events[index];
}
unsigned oracle_blend_logs(void)
{
	return blend_logs;
}
int oracle_blend_level(void)
{
	return blend_level;
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
